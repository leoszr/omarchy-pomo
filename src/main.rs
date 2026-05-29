mod cli;
mod daemon;
mod history;
mod ipc;
mod state;
mod task;
mod timer;
mod waybar;

use anyhow::bail;
use clap::Parser;

fn main() {
    if let Err(error) = run() {
        eprintln!("Erro: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    match args.command {
        Some(cli::Commands::Daemon) => daemon::run(state::StatePaths::new()?)?,
        Some(cli::Commands::Start(start_args)) => {
            let response = send(ipc::IpcRequest::Start { args: start_args })?;
            print_response(response)?;
        }
        Some(cli::Commands::Status(status_args)) => {
            if status_args.waybar {
                print_waybar_status()?;
            } else {
                print_response(send(ipc::IpcRequest::Status)?)?;
            }
        }
        Some(cli::Commands::Pause) => print_response(send(ipc::IpcRequest::Pause)?)?,
        Some(cli::Commands::Resume) => print_response(send(ipc::IpcRequest::Resume)?)?,
        Some(cli::Commands::Stop) => print_response(send(ipc::IpcRequest::Stop)?)?,
        Some(cli::Commands::History) => print_response(send(ipc::IpcRequest::History)?)?,
        Some(cli::Commands::Task { action }) => {
            task::run(&action);
        }
        None => {
            println!("Use --help para ver os comandos disponíveis.");
        }
    }
    Ok(())
}

fn send(request: ipc::IpcRequest) -> anyhow::Result<ipc::IpcResponse> {
    let paths = state::StatePaths::new()?;
    ipc::request(&paths, &request)
}

fn print_response(response: ipc::IpcResponse) -> anyhow::Result<()> {
    match response {
        ipc::IpcResponse::State { state } => println!("{}", format_status(&state)),
        ipc::IpcResponse::History { summary } => println!("{}", format_summary(&summary)),
        ipc::IpcResponse::Error { message } => bail!(message),
    }
    Ok(())
}

fn print_waybar_status() -> anyhow::Result<()> {
    let output = match send(ipc::IpcRequest::Status) {
        Ok(ipc::IpcResponse::State { state }) => waybar::from_state(&state, chrono::Local::now()),
        Ok(ipc::IpcResponse::Error { message }) => waybar::error(&message),
        Ok(ipc::IpcResponse::History { .. }) => waybar::error("resposta inesperada do daemon"),
        Err(error) => waybar::error(&format!("{error:#}")),
    };
    println!("{}", waybar::to_json(&output)?);
    Ok(())
}

fn format_status(current: &state::TimerState) -> String {
    let remaining = timer::remaining_secs_at(current, chrono::Local::now());
    format!(
        "status={} tipo={} label={} restante={}",
        status_name(&current.status),
        session_name(&current.session_type),
        current.label,
        format_duration(remaining)
    )
}

fn format_summary(summary: &history::DailySummary) -> String {
    format!(
        "data={} foco_sessoes={} foco_total={} pausas={}",
        summary.date,
        summary.focus_sessions,
        format_duration(summary.focused_secs),
        summary.break_sessions
    )
}

fn format_duration(secs: u64) -> String {
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

fn status_name(status: &state::TimerStatus) -> &'static str {
    match status {
        state::TimerStatus::Idle => "idle",
        state::TimerStatus::Running => "running",
        state::TimerStatus::Paused => "paused",
        state::TimerStatus::Finished => "finished",
    }
}

fn session_name(session_type: &state::SessionType) -> &'static str {
    match session_type {
        state::SessionType::Focus => "focus",
        state::SessionType::ShortBreak => "short_break",
        state::SessionType::Custom => "custom",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formata_duracao_mm_ss() {
        assert_eq!(format_duration(65), "01:05");
    }
}
