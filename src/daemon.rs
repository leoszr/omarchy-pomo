use std::{
    fs,
    os::unix::net::{UnixListener, UnixStream},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::Sender,
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use anyhow::Context;

use crate::{
    history,
    ipc::{self, IpcRequest, IpcResponse},
    notify::{CompletionNotifier, ExternalNotifier},
    state::{self, StatePaths, TimerState, TimerStatus},
    timer,
};

pub fn run(paths: StatePaths) -> anyhow::Result<()> {
    paths.ensure_base_dir()?;
    remove_stale_socket(&paths)?;
    let listener = UnixListener::bind(&paths.socket_file)
        .with_context(|| format!("falha ao criar socket {}", paths.socket_file.display()))?;
    listener
        .set_nonblocking(true)
        .context("falha ao configurar socket não bloqueante")?;
    println!("Daemon ouvindo em {}", paths.socket_file.display());

    run_loop(paths, listener)
}

fn run_loop(paths: StatePaths, listener: UnixListener) -> anyhow::Result<()> {
    run_loop_internal(paths, listener, None, None)
}

fn run_loop_internal(
    paths: StatePaths,
    listener: UnixListener,
    stop: Option<Arc<AtomicBool>>,
    accepted_tx: Option<Sender<()>>,
) -> anyhow::Result<()> {
    let mut notifier = ExternalNotifier;
    let request_lock = Arc::new(Mutex::new(()));
    let active_workers = Arc::new(AtomicUsize::new(0));

    loop {
        if stop
            .as_ref()
            .is_some_and(|should_stop| should_stop.load(Ordering::Relaxed))
        {
            break;
        }

        let now = chrono::Local::now();
        let tick_result = match request_lock.lock() {
            Ok(_guard) => tick_with_notifier(&paths, now, &mut notifier),
            Err(poisoned) => {
                eprintln!("Lock IPC envenenado; continuando com o estado recuperado");
                let _guard = poisoned.into_inner();
                tick_with_notifier(&paths, now, &mut notifier)
            }
        };
        let current = match tick_result {
            Ok(state) => Some(state),
            Err(error) => {
                eprintln!("Erro transitório no tick: {error:#}");
                None
            }
        };

        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    if !try_acquire_worker(&active_workers) {
                        eprintln!("Limite de workers IPC atingido; conexão rejeitada");
                        reject_connection(stream);
                        continue;
                    }

                    let paths = paths.clone();
                    let request_lock = Arc::clone(&request_lock);
                    let active_workers = Arc::clone(&active_workers);
                    if let Err(error) = stream
                        .set_read_timeout(Some(IPC_IO_TIMEOUT))
                        .and_then(|()| stream.set_write_timeout(Some(IPC_IO_TIMEOUT)))
                    {
                        active_workers.fetch_sub(1, Ordering::Release);
                        eprintln!("Falha ao configurar timeout IPC: {error:#}");
                        continue;
                    }

                    let worker_active_workers = Arc::clone(&active_workers);
                    let spawn_result =
                        thread::Builder::new()
                            .name("pomo-ipc".to_string())
                            .spawn(move || {
                                if let Err(error) = handle_stream(stream, &paths, &request_lock) {
                                    eprintln!("Erro IPC: {error:#}");
                                }
                                worker_active_workers.fetch_sub(1, Ordering::Release);
                            });
                    match spawn_result {
                        Ok(_) => {
                            let _ = accepted_tx.as_ref().map(|sender| sender.send(()));
                        }
                        Err(error) => {
                            active_workers.fetch_sub(1, Ordering::Release);
                            eprintln!("Falha ao criar worker IPC: {error:#}");
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => eprintln!("Erro ao aceitar conexão IPC: {error:#}"),
            }
        }

        let delay = current
            .as_ref()
            .map(|state| next_tick_delay(state, now))
            .unwrap_or(ERROR_RETRY_INTERVAL);
        thread::sleep(delay);
    }

    Ok(())
}

pub fn handle_request(paths: &StatePaths, request: IpcRequest) -> anyhow::Result<IpcResponse> {
    let mut notifier = ExternalNotifier;
    handle_request_at_with_notifier(paths, request, chrono::Local::now(), &mut notifier)
}

fn handle_request_at_with_notifier(
    paths: &StatePaths,
    request: IpcRequest,
    now: chrono::DateTime<chrono::Local>,
    notifier: &mut impl CompletionNotifier,
) -> anyhow::Result<IpcResponse> {
    match request {
        IpcRequest::Status => Ok(IpcResponse::State {
            state: reconcile_finished_state_at(paths, now, notifier)?,
        }),
        IpcRequest::Start { args } => {
            // Start não pode apagar uma sessão vencida sem primeiro registrá-la.
            reconcile_finished_state_at(paths, now, notifier)?;
            let state = timer::start_from_args(&args, now)?;
            state::write_state(paths, &state)?;
            Ok(IpcResponse::State { state })
        }
        IpcRequest::Pause => {
            update_state(paths, now, notifier, |state, now| timer::pause(&state, now))
        }
        IpcRequest::Resume => update_state(paths, now, notifier, |state, now| {
            timer::resume(&state, now)
        }),
        IpcRequest::Stop => {
            // Assim como Start, Stop precisa reconciliar antes de limpar o estado.
            reconcile_finished_state_at(paths, now, notifier)?;
            let state = timer::stop();
            state::write_state(paths, &state)?;
            Ok(IpcResponse::State { state })
        }
        IpcRequest::History => {
            reconcile_finished_state_at(paths, now, notifier)?;
            let entries = history::read_entries(paths)?;
            let summary = history::summarize_day(&entries, now.date_naive());
            Ok(IpcResponse::History { summary })
        }
    }
}

fn handle_stream(
    mut stream: UnixStream,
    paths: &StatePaths,
    request_lock: &Mutex<()>,
) -> anyhow::Result<()> {
    ipc::configure_timeouts(&stream)?;
    let response = match ipc::read_frame(&mut stream, ipc::MAX_REQUEST_BYTES, "request IPC") {
        Ok(raw) => match serde_json::from_slice::<IpcRequest>(&raw) {
            Ok(request) => {
                let _guard = request_lock
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                handle_request(paths, request).unwrap_or_else(|error| IpcResponse::Error {
                    message: format!("{error:#}"),
                })
            }
            Err(error) => IpcResponse::Error {
                message: format!("request IPC inválido: {error:#}"),
            },
        },
        Err(error) => IpcResponse::Error {
            message: format!("request IPC rejeitado: {error:#}"),
        },
    };
    write_response(&mut stream, &response)
}

fn reject_connection(mut stream: UnixStream) {
    let response = IpcResponse::Error {
        message: "daemon ocupado: limite de conexões atingido; tente novamente".to_string(),
    };
    let _ = ipc::configure_timeouts(&stream);
    if let Err(error) = write_response(&mut stream, &response) {
        eprintln!("Erro ao rejeitar conexão IPC: {error:#}");
    }
}

fn write_response(stream: &mut UnixStream, response: &IpcResponse) -> anyhow::Result<()> {
    let raw = serde_json::to_vec(response).context("falha ao serializar response IPC")?;
    if raw.len() <= ipc::MAX_RESPONSE_BYTES {
        return ipc::write_frame(stream, &raw, ipc::MAX_RESPONSE_BYTES, "response IPC");
    }

    let fallback = IpcResponse::Error {
        message: format!(
            "response IPC excede o limite de {} bytes",
            ipc::MAX_RESPONSE_BYTES
        ),
    };
    let fallback_raw = serde_json::to_vec(&fallback).context("falha ao serializar erro IPC")?;
    ipc::write_frame(
        stream,
        &fallback_raw,
        ipc::MAX_RESPONSE_BYTES,
        "response IPC",
    )
}

fn update_state(
    paths: &StatePaths,
    now: chrono::DateTime<chrono::Local>,
    notifier: &mut impl CompletionNotifier,
    update: impl FnOnce(TimerState, chrono::DateTime<chrono::Local>) -> TimerState,
) -> anyhow::Result<IpcResponse> {
    let current = reconcile_finished_state_at(paths, now, notifier)?;
    let updated = update(current, now);
    state::write_state(paths, &updated)?;
    Ok(IpcResponse::State { state: updated })
}

fn tick_with_notifier(
    paths: &StatePaths,
    now: chrono::DateTime<chrono::Local>,
    notifier: &mut impl CompletionNotifier,
) -> anyhow::Result<TimerState> {
    reconcile_finished_state_at(paths, now, notifier)
}

fn reconcile_finished_state_at(
    paths: &StatePaths,
    now: chrono::DateTime<chrono::Local>,
    notifier: &mut impl CompletionNotifier,
) -> anyhow::Result<TimerState> {
    let current = state::read_state(paths)?;
    let mut updated = timer::finish_if_due(&current, now);

    if updated.status == TimerStatus::Finished {
        // Commit point 1: make the terminal state durable before writing any
        // side effect.  A crash after this point is recovered below.
        state::write_state(paths, &updated)
            .context("sessão concluída, mas falha ao persistir estado Finished")?;

        let finished_at = timer::due_at(&current).unwrap_or(now);
        let entry = history::HistoryEntry::completed_from_state(&updated, finished_at);
        // Commit point 2: append is synced and identity-deduplicated.  If the
        // process dies before the marker write, the next call safely retries.
        history::append_entry(paths, &entry)
            .context("estado Finished preservado, mas falha ao persistir histórico")?;

        if !updated.history_recorded {
            updated.history_recorded = true;
            state::write_state(paths, &updated)
                .context("histórico persistido, mas falha ao confirmar marcador no estado")?;
        }

        if !updated.notification_sent {
            notifier
                .notify_completed(&updated)
                .context("histórico persistido, mas falha ao notificar conclusão")?;
            updated.notification_sent = true;
            state::write_state(paths, &updated).context(
                "conclusão e histórico persistidos, mas falha ao confirmar notificação; uma nova tentativa pode notificar novamente",
            )?;
        }
    } else if current != updated {
        state::write_state(paths, &updated)?;
    }
    Ok(updated)
}

const TICK_INTERVAL: Duration = Duration::from_millis(100);
const ERROR_RETRY_INTERVAL: Duration = Duration::from_secs(1);
const IPC_IO_TIMEOUT: Duration = Duration::from_millis(250);
const MAX_IPC_WORKERS: usize = 16;

fn try_acquire_worker(active_workers: &std::sync::atomic::AtomicUsize) -> bool {
    let mut active = active_workers.load(Ordering::Acquire);
    loop {
        if active >= MAX_IPC_WORKERS {
            return false;
        }
        match active_workers.compare_exchange_weak(
            active,
            active + 1,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => return true,
            Err(observed) => active = observed,
        }
    }
}

fn next_tick_delay(state: &TimerState, now: chrono::DateTime<chrono::Local>) -> Duration {
    let Some(due_at) = timer::due_at(state) else {
        return TICK_INTERVAL;
    };
    due_at
        .signed_duration_since(now)
        .to_std()
        .map(|delay| delay.min(TICK_INTERVAL))
        .unwrap_or(TICK_INTERVAL)
}

fn remove_stale_socket(paths: &StatePaths) -> anyhow::Result<()> {
    match UnixStream::connect(&paths.socket_file) {
        Ok(_) => anyhow::bail!(
            "daemon já parece estar rodando em {}",
            paths.socket_file.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => {}
    }

    match fs::remove_file(&paths.socket_file) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| {
            format!(
                "falha ao remover socket antigo {}",
                paths.socket_file.display()
            )
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cli::StartArgs,
        ipc::IpcRequest,
        state::{SessionType, TimerStatus},
    };
    use chrono::Duration;
    use std::fs;
    use std::os::unix::net::UnixListener;
    use std::time::{Duration as StdDuration, Instant};

    #[derive(Default)]
    struct MockNotifier {
        calls: usize,
    }

    impl crate::notify::CompletionNotifier for MockNotifier {
        fn notify_completed(&mut self, _state: &TimerState) -> anyhow::Result<()> {
            self.calls += 1;
            Ok(())
        }
    }

    struct FailingNotifier {
        calls: usize,
        failures_left: usize,
    }

    impl crate::notify::CompletionNotifier for FailingNotifier {
        fn notify_completed(&mut self, _state: &TimerState) -> anyhow::Result<()> {
            self.calls += 1;
            if self.failures_left > 0 {
                self.failures_left -= 1;
                anyhow::bail!("falha simulada de notificação")
            }
            Ok(())
        }
    }

    #[test]
    fn handler_status_altera_expirado_para_finished() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));
        let expired = timer::start_session(
            SessionType::Focus,
            "teste".to_string(),
            1,
            chrono::Local::now() - Duration::seconds(2),
        );
        state::write_state(&paths, &expired).unwrap();

        let response = handle_request(&paths, IpcRequest::Status).unwrap();

        let IpcResponse::State { state } = response else {
            panic!("response errada");
        };
        assert_eq!(state.status, TimerStatus::Finished);
        assert_eq!(history::read_entries(&paths).unwrap().len(), 1);
    }

    #[test]
    fn tick_finaliza_sem_consulta_de_cliente() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));
        let started_at = chrono::Local::now();
        let expired = timer::start_session(SessionType::Focus, "teste".to_string(), 60, started_at);
        state::write_state(&paths, &expired).unwrap();
        let mut notifier = MockNotifier::default();

        let state =
            tick_with_notifier(&paths, started_at + Duration::seconds(61), &mut notifier).unwrap();

        assert_eq!(state.status, TimerStatus::Finished);
        assert_eq!(notifier.calls, 1);
        assert_eq!(history::read_entries(&paths).unwrap().len(), 1);
    }

    #[test]
    fn cliente_preso_nao_impede_tick_do_loop() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));
        paths.ensure_base_dir().unwrap();
        let listener = UnixListener::bind(&paths.socket_file).unwrap();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let (accepted_tx, accepted_rx) = std::sync::mpsc::channel();
        let loop_stop = Arc::clone(&stop);
        let loop_paths = paths.clone();
        let scheduler = thread::spawn(move || {
            run_loop_internal(loop_paths, listener, Some(loop_stop), Some(accepted_tx))
        });

        let stuck_client = UnixStream::connect(&paths.socket_file).unwrap();
        accepted_rx
            .recv_timeout(StdDuration::from_secs(1))
            .expect("daemon não aceitou o cliente preso");

        let started_at = chrono::Local::now() - Duration::seconds(2);
        let expired = timer::start_session(
            SessionType::Focus,
            "cliente preso".to_string(),
            1,
            started_at,
        );
        state::write_state(&paths, &expired).unwrap();

        let deadline = Instant::now() + StdDuration::from_secs(2);
        loop {
            if state::read_state(&paths).unwrap().status == TimerStatus::Finished {
                break;
            }
            assert!(Instant::now() < deadline, "tick não finalizou o timer");
            thread::sleep(StdDuration::from_millis(10));
        }

        assert_eq!(history::read_entries(&paths).unwrap().len(), 1);
        drop(stuck_client);
        stop.store(true, Ordering::Relaxed);
        scheduler.join().unwrap().unwrap();
    }

    #[test]
    fn conexoes_presas_sao_limitadas_e_workers_se_recuperam() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));
        paths.ensure_base_dir().unwrap();
        let listener = UnixListener::bind(&paths.socket_file).unwrap();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let (accepted_tx, accepted_rx) = std::sync::mpsc::channel();
        let loop_stop = Arc::clone(&stop);
        let loop_paths = paths.clone();
        let scheduler = thread::spawn(move || {
            run_loop_internal(loop_paths, listener, Some(loop_stop), Some(accepted_tx))
        });

        let mut stuck_clients = Vec::new();
        for _ in 0..(MAX_IPC_WORKERS + 4) {
            stuck_clients.push(UnixStream::connect(&paths.socket_file).unwrap());
        }

        for _ in 0..MAX_IPC_WORKERS {
            accepted_rx
                .recv_timeout(StdDuration::from_secs(1))
                .expect("worker IPC não foi criado");
        }
        assert!(accepted_rx
            .recv_timeout(StdDuration::from_millis(50))
            .is_err());

        let expired = timer::start_session(
            SessionType::Focus,
            "limite IPC".to_string(),
            1,
            chrono::Local::now() - Duration::seconds(2),
        );
        state::write_state(&paths, &expired).unwrap();

        let deadline = Instant::now() + StdDuration::from_secs(2);
        loop {
            if state::read_state(&paths).unwrap().status == TimerStatus::Finished {
                break;
            }
            assert!(Instant::now() < deadline, "tick não finalizou o timer");
            thread::sleep(StdDuration::from_millis(10));
        }
        assert_eq!(history::read_entries(&paths).unwrap().len(), 1);

        let deadline = Instant::now() + StdDuration::from_secs(2);
        loop {
            if let Ok(IpcResponse::State { state }) =
                crate::ipc::request(&paths, &IpcRequest::Status)
            {
                assert_eq!(state.status, TimerStatus::Finished);
                break;
            }
            assert!(
                Instant::now() < deadline,
                "workers não se recuperaram após timeout"
            );
            thread::sleep(StdDuration::from_millis(10));
        }

        drop(stuck_clients);
        stop.store(true, Ordering::Relaxed);
        scheduler.join().unwrap().unwrap();
    }

    #[test]
    fn start_apos_vencimento_registra_sessao_anterior() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));
        let now = chrono::Local::now();
        let expired = timer::start_session(
            SessionType::Focus,
            "anterior".to_string(),
            60,
            now - Duration::seconds(61),
        );
        state::write_state(&paths, &expired).unwrap();

        let response = handle_request_at_with_notifier(
            &paths,
            IpcRequest::Start {
                args: StartArgs {
                    profile: Some("25-5".to_string()),
                    break_session: false,
                    custom: None,
                    session_type: None,
                },
            },
            now,
            &mut MockNotifier::default(),
        )
        .unwrap();

        let IpcResponse::State { state } = response else {
            panic!("response errada");
        };
        assert_eq!(state.status, TimerStatus::Running);
        assert_eq!(history::read_entries(&paths).unwrap().len(), 1);
    }

    #[test]
    fn stop_apos_vencimento_registra_sessao_anterior() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));
        let now = chrono::Local::now();
        let expired = timer::start_session(
            SessionType::Focus,
            "anterior".to_string(),
            60,
            now - Duration::seconds(61),
        );
        state::write_state(&paths, &expired).unwrap();

        let response = handle_request_at_with_notifier(
            &paths,
            IpcRequest::Stop,
            now,
            &mut MockNotifier::default(),
        )
        .unwrap();

        let IpcResponse::State { state } = response else {
            panic!("response errada");
        };
        assert_eq!(state.status, TimerStatus::Idle);
        assert_eq!(history::read_entries(&paths).unwrap().len(), 1);
    }

    #[test]
    fn historico_usa_data_do_vencimento_nao_da_deteccao() {
        use chrono::TimeZone;

        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));
        let started_at = chrono::Local
            .with_ymd_and_hms(2026, 1, 1, 23, 59, 59)
            .single()
            .unwrap();
        let expired = timer::start_session(SessionType::Focus, "virada".to_string(), 2, started_at);
        state::write_state(&paths, &expired).unwrap();
        let mut notifier = MockNotifier::default();

        tick_with_notifier(&paths, started_at + Duration::seconds(10), &mut notifier).unwrap();

        let entries = history::read_entries(&paths).unwrap();
        assert_eq!(entries[0].finished_at, started_at + Duration::seconds(2));
        assert_eq!(
            entries[0].date,
            (started_at + Duration::seconds(2)).date_naive()
        );
    }

    #[test]
    fn finalizacao_chama_notificacao_uma_unica_vez() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));
        let expired = timer::start_session(
            SessionType::Focus,
            "teste".to_string(),
            1,
            chrono::Local::now() - Duration::seconds(2),
        );
        state::write_state(&paths, &expired).unwrap();
        let mut notifier = MockNotifier::default();

        tick_with_notifier(&paths, chrono::Local::now(), &mut notifier).unwrap();
        tick_with_notifier(&paths, chrono::Local::now(), &mut notifier).unwrap();

        assert_eq!(notifier.calls, 1);
        assert_eq!(history::read_entries(&paths).unwrap().len(), 1);
    }

    #[test]
    fn falha_de_notificacao_preserva_finished_e_reprocessa_sem_duplicar() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));
        let expired = timer::start_session(
            SessionType::Focus,
            "teste".to_string(),
            1,
            chrono::Local::now() - Duration::seconds(2),
        );
        state::write_state(&paths, &expired).unwrap();
        let mut notifier = FailingNotifier {
            calls: 0,
            failures_left: 1,
        };

        assert!(reconcile_finished_state_at(&paths, chrono::Local::now(), &mut notifier).is_err());
        let after_failure = state::read_state(&paths).unwrap();
        assert_eq!(after_failure.status, TimerStatus::Finished);
        assert!(after_failure.history_recorded);
        assert!(!after_failure.notification_sent);
        assert_eq!(history::read_entries(&paths).unwrap().len(), 1);

        let recovered =
            reconcile_finished_state_at(&paths, chrono::Local::now(), &mut notifier).unwrap();

        assert!(recovered.notification_sent);
        assert_eq!(notifier.calls, 2);
        assert_eq!(history::read_entries(&paths).unwrap().len(), 1);
    }

    #[test]
    fn estado_e_historico_legados_sao_recuperados() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));
        paths.ensure_base_dir().unwrap();
        let now = chrono::Local::now();
        let started_at = now - Duration::seconds(2);
        let legacy_state = serde_json::json!({
            "status": "running",
            "session_type": "focus",
            "label": "legado",
            "duration_secs": 1,
            "started_at": started_at,
            "paused_remaining_secs": null
        });
        let legacy_entry = serde_json::json!({
            "date": now.date_naive(),
            "type": "focus",
            "label": "legado",
            "duration_secs": 1,
            "completed": true,
            "finished_at": now
        });
        fs::write(
            &paths.state_file,
            serde_json::to_vec(&legacy_state).unwrap(),
        )
        .unwrap();
        fs::write(
            &paths.history_file,
            format!("{}\n", serde_json::to_string(&legacy_entry).unwrap()),
        )
        .unwrap();
        let mut notifier = MockNotifier::default();

        let state = reconcile_finished_state_at(&paths, now, &mut notifier).unwrap();

        assert_eq!(state.status, TimerStatus::Finished);
        assert!(state.session_id.is_some());
        // The old line is retained: its lack of identity makes it unsafe to
        // assume that it is the just-finished session.
        assert_eq!(history::read_entries(&paths).unwrap().len(), 2);
        assert_eq!(notifier.calls, 1);
    }

    #[test]
    fn handler_start_grava_estado() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));

        handle_request(
            &paths,
            IpcRequest::Start {
                args: StartArgs {
                    profile: Some("25-5".to_string()),
                    break_session: false,
                    custom: None,
                    session_type: None,
                },
            },
        )
        .unwrap();

        assert_eq!(
            state::read_state(&paths).unwrap().status,
            TimerStatus::Running
        );
    }

    #[test]
    fn nao_remove_socket_de_daemon_ativo() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));
        paths.ensure_base_dir().unwrap();
        let _listener = UnixListener::bind(&paths.socket_file).unwrap();

        let error = remove_stale_socket(&paths).unwrap_err().to_string();

        assert!(error.contains("daemon já parece estar rodando"));
        assert!(paths.socket_file.exists());
    }

    #[test]
    fn remove_socket_stale() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));
        paths.ensure_base_dir().unwrap();
        let listener = UnixListener::bind(&paths.socket_file).unwrap();
        drop(listener);

        remove_stale_socket(&paths).unwrap();

        assert!(!paths.socket_file.exists());
    }
}
