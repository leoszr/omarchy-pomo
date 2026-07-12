use std::{
    fs,
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    thread,
    time::Duration,
};

use anyhow::Context;

use crate::{
    history,
    ipc::{IpcRequest, IpcResponse},
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

    let mut notifier = ExternalNotifier;
    loop {
        let now = chrono::Local::now();
        let current = tick_with_notifier(&paths, now, &mut notifier)?;

        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    if let Err(error) = handle_stream(stream, &paths) {
                        eprintln!("Erro IPC: {error:#}");
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => eprintln!("Erro ao aceitar conexão IPC: {error:#}"),
            }
        }

        thread::sleep(next_tick_delay(&current, now));
    }
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

fn handle_stream(mut stream: UnixStream, paths: &StatePaths) -> anyhow::Result<()> {
    let mut raw = String::new();
    stream
        .read_to_string(&mut raw)
        .context("falha ao ler request IPC")?;
    let response = match serde_json::from_str::<IpcRequest>(&raw) {
        Ok(request) => handle_request(paths, request).unwrap_or_else(|error| IpcResponse::Error {
            message: format!("{error:#}"),
        }),
        Err(error) => IpcResponse::Error {
            message: format!("request IPC inválido: {error:#}"),
        },
    };
    let raw_response = serde_json::to_vec(&response).context("falha ao serializar response IPC")?;
    stream
        .write_all(&raw_response)
        .context("falha ao enviar response IPC")
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
    let updated = timer::finish_if_due(&current, now);

    if current.status != TimerStatus::Finished && updated.status == TimerStatus::Finished {
        let finished_at = timer::due_at(&current).unwrap_or(now);
        let entry = history::HistoryEntry::completed_from_state(&updated, finished_at);
        history::append_entry(paths, &entry)?;
        notifier.notify_completed(&updated);
    }

    if current != updated {
        state::write_state(paths, &updated)?;
    }
    Ok(updated)
}

const TICK_INTERVAL: Duration = Duration::from_millis(100);

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
    use std::os::unix::net::UnixListener;

    #[derive(Default)]
    struct MockNotifier {
        calls: usize,
    }

    impl crate::notify::CompletionNotifier for MockNotifier {
        fn notify_completed(&mut self, _state: &TimerState) {
            self.calls += 1;
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
