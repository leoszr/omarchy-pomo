mod cli;
mod state;
mod task;
mod timer;

use clap::Parser;

fn main() {
    let args = cli::Cli::parse();
    match args.command {
        Some(cli::Commands::Start) => match state::StatePaths::new().and_then(|paths| {
            let current = timer::start_focus_25(chrono::Local::now());
            state::write_state(&paths, &current)
        }) {
            Ok(()) => println!("Pomodoro iniciado: 25/5 Focus"),
            Err(error) => eprintln!("Erro ao iniciar pomodoro: {error:#}"),
        },
        Some(cli::Commands::Status) => {
            match state::StatePaths::new().and_then(|paths| state::read_state(&paths)) {
                Ok(current) => println!(
                    "status={:?} tipo={:?} label={} restante={}s",
                    current.status,
                    current.session_type,
                    current.label,
                    timer::remaining_secs_at(&current, chrono::Local::now())
                ),
                Err(error) => eprintln!("Erro ao ler estado: {error:#}"),
            }
        }
        Some(cli::Commands::Task { action }) => {
            task::run(&action);
        }
        None => {
            println!("Use --help para ver os comandos disponíveis.");
        }
    }
}
