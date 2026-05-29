use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::{
    cli::StartArgs,
    history::DailySummary,
    state::{StatePaths, TimerState},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum IpcRequest {
    Status,
    Start { args: StartArgs },
    Pause,
    Resume,
    Stop,
    History,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcResponse {
    State { state: TimerState },
    History { summary: DailySummary },
    Error { message: String },
}

pub fn request(paths: &StatePaths, request: &IpcRequest) -> anyhow::Result<IpcResponse> {
    let mut stream = UnixStream::connect(&paths.socket_file).with_context(|| {
        format!(
            "daemon indisponível em {}; rode `omarchy-pomo daemon`",
            paths.socket_file.display()
        )
    })?;

    let raw = serde_json::to_vec(request).context("falha ao serializar request IPC")?;
    stream
        .write_all(&raw)
        .context("falha ao enviar request IPC")?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .context("falha ao finalizar escrita IPC")?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .context("falha ao ler response IPC")?;
    serde_json::from_str(&response).context("falha ao interpretar response IPC")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::CustomSessionType;

    #[test]
    fn serializa_request_start() {
        let request = IpcRequest::Start {
            args: StartArgs {
                profile: None,
                break_session: false,
                custom: Some(10),
                session_type: Some(CustomSessionType::Focus),
            },
        };

        let raw = serde_json::to_string(&request).unwrap();
        let parsed: IpcRequest = serde_json::from_str(&raw).unwrap();

        assert_eq!(parsed, request);
    }

    #[test]
    fn serializa_response_error() {
        let response = IpcResponse::Error {
            message: "erro".to_string(),
        };

        let raw = serde_json::to_string(&response).unwrap();
        let parsed: IpcResponse = serde_json::from_str(&raw).unwrap();

        assert_eq!(parsed, response);
    }

    #[test]
    fn erro_amigavel_quando_socket_nao_existe() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));

        let error = request(&paths, &IpcRequest::Status)
            .unwrap_err()
            .to_string();

        assert!(error.contains("daemon indisponível"));
        assert!(error.contains("omarchy-pomo daemon"));
    }
}
