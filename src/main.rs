mod cli;
mod state;
mod task;
mod timer;

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
        Some(cli::Commands::Start(start_args)) => {
            let paths = state::StatePaths::new()?;
            let current = timer::start_from_args(&start_args, chrono::Local::now())?;
            state::write_state(&paths, &current)?;
            println!("Sessão iniciada: {}", current.label);
        }
        Some(cli::Commands::Status) => {
            let paths = state::StatePaths::new()?;
            let current = state::read_state(&paths)?;
            let current = timer::finish_if_due(&current, chrono::Local::now());
            state::write_state(&paths, &current)?;
            println!("{}", format_status(&current));
        }
        Some(cli::Commands::Pause) => update_state(|current, now| timer::pause(&current, now))?,
        Some(cli::Commands::Resume) => update_state(|current, now| timer::resume(&current, now))?,
        Some(cli::Commands::Stop) => {
            let paths = state::StatePaths::new()?;
            state::write_state(&paths, &timer::stop())?;
            println!("Sessão parada.");
        }
        Some(cli::Commands::Task { action }) => {
            task::run(&action);
        }
        None => {
            println!("Use --help para ver os comandos disponíveis.");
        }
    }
    Ok(())
}

fn update_state(
    update: impl FnOnce(state::TimerState, chrono::DateTime<chrono::Local>) -> state::TimerState,
) -> anyhow::Result<()> {
    let paths = state::StatePaths::new()?;
    let current = state::read_state(&paths)?;
    let updated = update(current, chrono::Local::now());
    state::write_state(&paths, &updated)?;
    println!("{}", format_status(&updated));
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
