use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "omarchy-pomo")]
#[command(about = "CLI inicial para Pomodoro", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Inicia um ciclo de pomodoro
    Start,
    /// Mostra o estado persistido local
    Status,
    /// Gerencia tarefas
    Task {
        #[arg(default_value = "list")]
        action: String,
    },
}
