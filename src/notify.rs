use std::{
    io,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus},
    sync::Arc,
    thread::{self, JoinHandle},
};

use crate::state::TimerState;

const MAX_PENDING_WORKERS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
}

pub trait CommandProcess: Send {
    fn wait(self: Box<Self>) -> io::Result<ExitStatus>;
}

pub trait CommandSpawner: Send + Sync {
    fn spawn(&self, spec: &CommandSpec) -> io::Result<Box<dyn CommandProcess>>;
}

#[derive(Debug, Default)]
pub struct SystemCommandSpawner;

struct SystemCommandProcess {
    child: Child,
}

impl CommandSpawner for SystemCommandSpawner {
    fn spawn(&self, spec: &CommandSpec) -> io::Result<Box<dyn CommandProcess>> {
        let child = Command::new(&spec.program).args(&spec.args).spawn()?;
        Ok(Box::new(SystemCommandProcess { child }))
    }
}

impl CommandProcess for SystemCommandProcess {
    fn wait(mut self: Box<Self>) -> io::Result<ExitStatus> {
        let result = self.child.wait();
        if result.is_err() {
            // Child::drop does not wait. Try to terminate and reap it if wait
            // itself failed, so a best-effort worker cannot leave a zombie.
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        result
    }
}

pub trait CompletionNotifier {
    fn notify_completed(&mut self, state: &TimerState) -> anyhow::Result<()>;
}

pub struct ExternalNotifier {
    spawner: Arc<dyn CommandSpawner>,
    workers: Vec<JoinHandle<()>>,
}

impl Default for ExternalNotifier {
    fn default() -> Self {
        Self::with_spawner(Arc::new(SystemCommandSpawner))
    }
}

impl ExternalNotifier {
    pub fn with_spawner(spawner: Arc<dyn CommandSpawner>) -> Self {
        Self {
            spawner,
            workers: Vec::new(),
        }
    }

    #[cfg(test)]
    fn shutdown(mut self) {
        self.join_workers();
    }

    fn join_workers(&mut self) {
        for worker in std::mem::take(&mut self.workers) {
            if worker.join().is_err() {
                eprintln!("notificação: worker terminou com panic");
            }
        }
    }

    fn reap_finished(&mut self) {
        let mut pending = Vec::with_capacity(self.workers.len());
        for worker in std::mem::take(&mut self.workers) {
            if worker.is_finished() {
                if worker.join().is_err() {
                    eprintln!("notificação: worker terminou com panic");
                }
            } else {
                pending.push(worker);
            }
        }
        self.workers = pending;
    }

    fn enqueue<F>(&mut self, name: &'static str, job: F)
    where
        F: FnOnce(Arc<dyn CommandSpawner>) + Send + 'static,
    {
        self.reap_finished();
        if self.workers.len() >= MAX_PENDING_WORKERS {
            eprintln!("notificação: limite de {MAX_PENDING_WORKERS} workers pendentes atingido");
            return;
        }

        let spawner = Arc::clone(&self.spawner);
        match thread::Builder::new()
            .name(name.to_string())
            .spawn(move || job(spawner))
        {
            Ok(worker) => self.workers.push(worker),
            Err(error) => eprintln!("notificação: falha ao criar worker {name}: {error}"),
        }
    }

    fn enqueue_command(&mut self, name: &'static str, spec: CommandSpec) {
        self.enqueue(name, move |spawner| {
            run_command(&*spawner, &spec);
        });
    }
}

impl Drop for ExternalNotifier {
    fn drop(&mut self) {
        // Dropping a JoinHandle detaches its thread. Join here so an implicit
        // shutdown has the same no-zombie guarantee as the explicit one. The
        // daemon keeps this object alive for its whole listener lifetime, so
        // this never runs on the IPC request path.
        self.join_workers();
    }
}

impl CompletionNotifier for ExternalNotifier {
    fn notify_completed(&mut self, state: &TimerState) -> anyhow::Result<()> {
        self.enqueue_command("omarchy-pomo-notify", notify_send_command(state));

        if let Some(sound_file) = sound_file_path().filter(|path| path.exists()) {
            self.enqueue("omarchy-pomo-audio", move |spawner| {
                run_audio(&*spawner, &sound_file);
            });
        }
        Ok(())
    }
}

pub fn notify_send_command(state: &TimerState) -> CommandSpec {
    CommandSpec {
        program: "notify-send".to_string(),
        args: vec![
            "Pomodoro finalizado".to_string(),
            format!("Sessão concluída: {}", state.label),
        ],
    }
}

pub fn paplay_command(sound_file: &Path) -> CommandSpec {
    CommandSpec {
        program: "paplay".to_string(),
        args: vec![sound_file.display().to_string()],
    }
}

pub fn mpv_command(sound_file: &Path) -> CommandSpec {
    CommandSpec {
        program: "mpv".to_string(),
        args: vec!["--no-video".to_string(), sound_file.display().to_string()],
    }
}

pub fn sound_file_path() -> Option<PathBuf> {
    let config_root =
        dirs::config_dir().or_else(|| dirs::home_dir().map(|home| home.join(".config")))?;
    Some(config_root.join("omarchy-pomo/done.ogg"))
}

fn run_audio(spawner: &dyn CommandSpawner, sound_file: &Path) -> bool {
    if run_command(spawner, &paplay_command(sound_file)) {
        return true;
    }
    run_command(spawner, &mpv_command(sound_file))
}

fn run_command(spawner: &dyn CommandSpawner, spec: &CommandSpec) -> bool {
    let process = match spawner.spawn(spec) {
        Ok(process) => process,
        Err(error) => {
            eprintln!(
                "notificação: não foi possível iniciar {}: {error}",
                spec.program
            );
            return false;
        }
    };

    match process.wait() {
        Ok(status) if status.success() => true,
        Ok(status) => {
            eprintln!("notificação: {} terminou com status {status}", spec.program);
            false
        }
        Err(error) => {
            eprintln!("notificação: falha aguardando {}: {error}", spec.program);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{SessionType, TimerStatus};
    use std::{
        collections::VecDeque,
        os::unix::process::ExitStatusExt,
        sync::{mpsc, Mutex},
        time::{Duration, Instant},
    };

    fn finished_state() -> TimerState {
        TimerState {
            status: TimerStatus::Finished,
            session_type: SessionType::Focus,
            label: "25/5 Focus".to_string(),
            duration_secs: 1_500,
            started_at: None,
            paused_remaining_secs: Some(0),
            session_id: Some("session-test".to_string()),
            history_recorded: true,
            notification_sent: true,
        }
    }

    #[test]
    fn constroi_comando_notify_send() {
        let command = notify_send_command(&finished_state());

        assert_eq!(command.program, "notify-send");
        assert_eq!(command.args[0], "Pomodoro finalizado");
        assert!(command.args[1].contains("25/5 Focus"));
    }

    #[test]
    fn constroi_comandos_de_som() {
        let path = PathBuf::from("/tmp/done.ogg");

        assert_eq!(paplay_command(&path).args, vec!["/tmp/done.ogg"]);
        assert_eq!(mpv_command(&path).args, vec!["--no-video", "/tmp/done.ogg"]);
    }

    #[test]
    fn ausencia_de_arquivo_de_som_nao_gera_erro() {
        let path = PathBuf::from("/tmp/omarchy-pomo-test-no-such-file.ogg");
        assert!(!path.exists());
    }

    struct MockProcess {
        status: ExitStatus,
    }

    impl CommandProcess for MockProcess {
        fn wait(self: Box<Self>) -> io::Result<ExitStatus> {
            Ok(self.status)
        }
    }

    struct MockSpawner {
        outcomes: Mutex<VecDeque<MockOutcome>>,
        calls: Mutex<Vec<CommandSpec>>,
    }

    enum MockOutcome {
        SpawnFailure,
        Exit(bool),
    }

    impl MockSpawner {
        fn with_outcomes(outcomes: impl IntoIterator<Item = bool>) -> Self {
            Self {
                outcomes: Mutex::new(outcomes.into_iter().map(MockOutcome::Exit).collect()),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn with_results(outcomes: impl IntoIterator<Item = MockOutcome>) -> Self {
            Self {
                outcomes: Mutex::new(outcomes.into_iter().collect()),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<CommandSpec> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl CommandSpawner for MockSpawner {
        fn spawn(&self, spec: &CommandSpec) -> io::Result<Box<dyn CommandProcess>> {
            self.calls.lock().unwrap().push(spec.clone());
            let outcome = self
                .outcomes
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(MockOutcome::Exit(true));
            let MockOutcome::Exit(success) = outcome else {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "mock command absent",
                ));
            };
            let code = if success { 0 } else { 1 };
            Ok(Box::new(MockProcess {
                status: ExitStatus::from_raw(code),
            }))
        }
    }

    struct BlockingProcess {
        release: Arc<std::sync::atomic::AtomicBool>,
    }

    impl CommandProcess for BlockingProcess {
        fn wait(self: Box<Self>) -> io::Result<ExitStatus> {
            while !self.release.load(std::sync::atomic::Ordering::Acquire) {
                thread::sleep(Duration::from_millis(1));
            }
            Ok(ExitStatus::from_raw(0))
        }
    }

    struct BlockingSpawner {
        started: Mutex<Option<mpsc::Sender<()>>>,
        release: Arc<std::sync::atomic::AtomicBool>,
    }

    impl CommandSpawner for BlockingSpawner {
        fn spawn(&self, _spec: &CommandSpec) -> io::Result<Box<dyn CommandProcess>> {
            let sender = self.started.lock().unwrap().take().unwrap();
            sender.send(()).unwrap();
            Ok(Box::new(BlockingProcess {
                release: Arc::clone(&self.release),
            }))
        }
    }

    #[test]
    fn paplay_falho_faz_fallback_para_mpv_sem_sobreposicao() {
        let spawner = MockSpawner::with_outcomes([false, true]);
        assert!(run_audio(&spawner, Path::new("/tmp/done.ogg")));

        let calls = spawner.calls();
        assert_eq!(
            calls
                .iter()
                .map(|call| call.program.as_str())
                .collect::<Vec<_>>(),
            vec!["paplay", "mpv"]
        );
    }

    #[test]
    fn notify_nao_espera_o_processo_filho() {
        let (started_tx, started_rx) = mpsc::channel();
        let release = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let spawner = Arc::new(BlockingSpawner {
            started: Mutex::new(Some(started_tx)),
            release: Arc::clone(&release),
        });
        let mut notifier = ExternalNotifier::with_spawner(spawner);
        let started = Instant::now();
        notifier.enqueue_command("test-notification", notify_send_command(&finished_state()));

        assert!(started.elapsed() < Duration::from_millis(100));
        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("worker não iniciou o processo");
        release.store(true, std::sync::atomic::Ordering::Release);
        notifier.shutdown();
    }

    #[test]
    fn spawn_falho_tambem_permite_fallback() {
        let spawner =
            MockSpawner::with_results([MockOutcome::SpawnFailure, MockOutcome::Exit(true)]);
        assert!(run_audio(&spawner, Path::new("/tmp/done.ogg")));
        assert_eq!(spawner.calls().len(), 2);
    }

    #[test]
    fn workers_concluidos_sao_reapados_sem_crescimento_da_colecao() {
        let spawner = Arc::new(MockSpawner::with_outcomes(std::iter::repeat_n(true, 100)));
        let mut notifier = ExternalNotifier::with_spawner(spawner.clone());

        for _ in 0..100 {
            notifier.enqueue_command("test-notification", notify_send_command(&finished_state()));
            assert!(notifier.workers.len() <= MAX_PENDING_WORKERS);

            while notifier.workers.iter().any(|worker| !worker.is_finished()) {
                notifier.reap_finished();
                thread::yield_now();
            }
            notifier.reap_finished();
        }

        assert!(notifier.workers.is_empty());
        assert_eq!(spawner.calls().len(), 100);
        notifier.shutdown();
    }
}
