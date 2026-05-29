use std::{
    fs,
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
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
    println!("Daemon ouvindo em {}", paths.socket_file.display());

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_stream(stream, &paths) {
                    eprintln!("Erro IPC: {error:#}");
                }
            }
            Err(error) => eprintln!("Erro ao aceitar conexão IPC: {error:#}"),
        }
    }

    Ok(())
}

pub fn handle_request(paths: &StatePaths, request: IpcRequest) -> anyhow::Result<IpcResponse> {
    match request {
        IpcRequest::Status => Ok(IpcResponse::State {
            state: refresh_finished_state(paths)?,
        }),
        IpcRequest::Start { args } => {
            let state = timer::start_from_args(&args, chrono::Local::now())?;
            state::write_state(paths, &state)?;
            Ok(IpcResponse::State { state })
        }
        IpcRequest::Pause => update_state(paths, |state, now| timer::pause(&state, now)),
        IpcRequest::Resume => update_state(paths, |state, now| timer::resume(&state, now)),
        IpcRequest::Stop => {
            let state = timer::stop();
            state::write_state(paths, &state)?;
            Ok(IpcResponse::State { state })
        }
        IpcRequest::History => {
            refresh_finished_state(paths)?;
            let entries = history::read_entries(paths)?;
            let summary = history::summarize_day(&entries, chrono::Local::now().date_naive());
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
    update: impl FnOnce(TimerState, chrono::DateTime<chrono::Local>) -> TimerState,
) -> anyhow::Result<IpcResponse> {
    let current = refresh_finished_state(paths)?;
    let updated = update(current, chrono::Local::now());
    state::write_state(paths, &updated)?;
    Ok(IpcResponse::State { state: updated })
}

fn refresh_finished_state(paths: &StatePaths) -> anyhow::Result<TimerState> {
    let mut notifier = ExternalNotifier;
    refresh_finished_state_with_notifier(paths, &mut notifier)
}

fn refresh_finished_state_with_notifier(
    paths: &StatePaths,
    notifier: &mut impl CompletionNotifier,
) -> anyhow::Result<TimerState> {
    let now = chrono::Local::now();
    let current = state::read_state(paths)?;
    let updated = timer::finish_if_due(&current, now);

    if current.status != TimerStatus::Finished && updated.status == TimerStatus::Finished {
        let entry = history::HistoryEntry::completed_from_state(&updated, now);
        history::append_entry(paths, &entry)?;
        notifier.notify_completed(&updated);
    }

    state::write_state(paths, &updated)?;
    Ok(updated)
}

fn remove_stale_socket(paths: &StatePaths) -> anyhow::Result<()> {
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

        refresh_finished_state_with_notifier(&paths, &mut notifier).unwrap();
        refresh_finished_state_with_notifier(&paths, &mut notifier).unwrap();

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
}
