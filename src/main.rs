mod cli;
mod history;
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
            let current = refresh_finished_state(&paths)?;
            println!("{}", format_status(&current));
        }
        Some(cli::Commands::Pause) => update_state(|current, now| timer::pause(&current, now))?,
        Some(cli::Commands::Resume) => update_state(|current, now| timer::resume(&current, now))?,
        Some(cli::Commands::Stop) => {
            let paths = state::StatePaths::new()?;
            state::write_state(&paths, &timer::stop())?;
            println!("Sessão parada.");
        }
        Some(cli::Commands::History) => {
            let paths = state::StatePaths::new()?;
            let entries = history::read_entries(&paths)?;
            let summary = history::summarize_day(&entries, chrono::Local::now().date_naive());
            println!("{}", format_summary(&summary));
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

fn refresh_finished_state(paths: &state::StatePaths) -> anyhow::Result<state::TimerState> {
    let now = chrono::Local::now();
    let current = state::read_state(paths)?;
    let updated = timer::finish_if_due(&current, now);

    if current.status != state::TimerStatus::Finished
        && updated.status == state::TimerStatus::Finished
    {
        let entry = history::HistoryEntry::completed_from_state(&updated, now);
        history::append_entry(paths, &entry)?;
    }

    state::write_state(paths, &updated)?;
    Ok(updated)
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
    use chrono::Duration;

    #[test]
    fn formata_duracao_mm_ss() {
        assert_eq!(format_duration(65), "01:05");
    }

    #[test]
    fn stop_manual_nao_escreve_historico() {
        let dir = tempfile::tempdir().unwrap();
        let paths = state::StatePaths::from_base(dir.path().join("omarchy-pomo"));
        let running = timer::start_session(
            state::SessionType::Focus,
            "teste".to_string(),
            60,
            chrono::Local::now(),
        );
        state::write_state(&paths, &running).unwrap();

        state::write_state(&paths, &timer::stop()).unwrap();

        assert!(history::read_entries(&paths).unwrap().is_empty());
    }

    #[test]
    fn finalizacao_repetida_nao_duplica_historico() {
        let dir = tempfile::tempdir().unwrap();
        let paths = state::StatePaths::from_base(dir.path().join("omarchy-pomo"));
        let expired = timer::start_session(
            state::SessionType::Focus,
            "teste".to_string(),
            1,
            chrono::Local::now() - Duration::seconds(2),
        );
        state::write_state(&paths, &expired).unwrap();

        refresh_finished_state(&paths).unwrap();
        refresh_finished_state(&paths).unwrap();

        assert_eq!(history::read_entries(&paths).unwrap().len(), 1);
    }
}
