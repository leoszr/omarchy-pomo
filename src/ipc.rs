use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::Duration,
};

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

use crate::{
    cli::StartArgs,
    history::DailySummary,
    state::{StatePaths, TimerState},
};

/// Limite do payload JSON, sem contar o byte de framing (`\n`).
pub(crate) const MAX_REQUEST_BYTES: usize = 64 * 1024;
pub(crate) const MAX_RESPONSE_BYTES: usize = 64 * 1024;
pub(crate) const IPC_READ_TIMEOUT: Duration = Duration::from_secs(2);
pub(crate) const IPC_WRITE_TIMEOUT: Duration = Duration::from_secs(2);

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
    configure_timeouts(&stream)?;

    let raw = serde_json::to_vec(request).context("falha ao serializar request IPC")?;
    write_frame(&mut stream, &raw, MAX_REQUEST_BYTES, "request IPC")
        .context("falha ao enviar request IPC")?;

    let response = read_frame(&mut stream, MAX_RESPONSE_BYTES, "response IPC")
        .context("falha ao ler response IPC")?;
    serde_json::from_slice(&response).context("falha ao interpretar response IPC")
}

pub(crate) fn configure_timeouts(stream: &UnixStream) -> anyhow::Result<()> {
    stream
        .set_read_timeout(Some(IPC_READ_TIMEOUT))
        .context("falha ao configurar timeout de leitura IPC")?;
    stream
        .set_write_timeout(Some(IPC_WRITE_TIMEOUT))
        .context("falha ao configurar timeout de escrita IPC")?;
    Ok(())
}

/// Lê exatamente um frame JSON delimitado por newline sem crescer além do limite.
pub(crate) fn read_frame(
    stream: &mut UnixStream,
    max_bytes: usize,
    name: &str,
) -> anyhow::Result<Vec<u8>> {
    let mut payload = Vec::with_capacity(max_bytes.min(4096));
    let mut buffer = [0_u8; 4096];

    loop {
        let bytes_read = match stream.read(&mut buffer) {
            Ok(bytes_read) => bytes_read,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                bail!("timeout lendo {name} após {}s", IPC_READ_TIMEOUT.as_secs())
            }
            Err(error) => return Err(error).with_context(|| format!("falha ao ler {name}")),
        };

        if bytes_read == 0 {
            if payload.is_empty() {
                bail!("{name} vazio (EOF)");
            }
            bail!("EOF antes do delimitador newline em {name}");
        }

        for &byte in &buffer[..bytes_read] {
            if byte == b'\n' {
                if payload.iter().all(u8::is_ascii_whitespace) {
                    bail!("{name} vazio");
                }
                return Ok(payload);
            }
            if payload.len() >= max_bytes {
                bail!("{name} excede o limite de {max_bytes} bytes");
            }
            payload.push(byte);
        }
    }
}

pub(crate) fn write_frame(
    stream: &mut UnixStream,
    payload: &[u8],
    max_bytes: usize,
    name: &str,
) -> anyhow::Result<()> {
    if payload.len() > max_bytes {
        bail!("{name} excede o limite de {max_bytes} bytes");
    }
    stream
        .write_all(payload)
        .with_context(|| format!("falha ao enviar {name}"))?;
    stream
        .write_all(b"\n")
        .with_context(|| format!("falha ao finalizar {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::CustomSessionType;
    use std::{io::Write, os::unix::net::UnixStream, time::Duration};

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

    #[test]
    fn frame_trata_eof_payload_vazio_e_excessivo() {
        let (mut server, client) = UnixStream::pair().unwrap();
        drop(client);
        let error = read_frame(&mut server, MAX_REQUEST_BYTES, "request IPC")
            .unwrap_err()
            .to_string();
        assert!(error.contains("vazio") && error.contains("EOF"));

        let (mut server, mut client) = UnixStream::pair().unwrap();
        client.write_all(b"{}").unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        let error = read_frame(&mut server, MAX_REQUEST_BYTES, "request IPC")
            .unwrap_err()
            .to_string();
        assert!(error.contains("EOF antes"));

        let (mut server, mut client) = UnixStream::pair().unwrap();
        client
            .write_all(&vec![b'x'; MAX_REQUEST_BYTES + 1])
            .unwrap();
        let error = read_frame(&mut server, MAX_REQUEST_BYTES, "request IPC")
            .unwrap_err()
            .to_string();
        assert!(error.contains("excede o limite"));
    }

    #[test]
    fn frame_respeita_timeout_de_leitura() {
        let (mut server, _client) = UnixStream::pair().unwrap();
        server
            .set_read_timeout(Some(Duration::from_millis(20)))
            .unwrap();

        let error = read_frame(&mut server, MAX_REQUEST_BYTES, "request IPC")
            .unwrap_err()
            .to_string();
        assert!(error.contains("timeout lendo request IPC"));
    }
}
