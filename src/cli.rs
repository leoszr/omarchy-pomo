use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "omarchy-pomo")]
#[command(about = "Pomodoro local para Omarchy/Hyprland", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Inicia uma sessão de pomodoro
    Start(StartArgs),
    /// Mostra o estado persistido local
    Status,
    /// Pausa a sessão atual
    Pause,
    /// Retoma a sessão pausada
    Resume,
    /// Para a sessão atual e volta para idle
    Stop,
    /// Mostra resumo do histórico de hoje
    History,
    /// Gerencia tarefas
    Task {
        #[arg(default_value = "list")]
        action: String,
    },
}

#[derive(Args, Debug)]
pub struct StartArgs {
    /// Perfil de foco: 25-5 ou 30-10
    #[arg(long, value_parser = ["25-5", "30-10"], conflicts_with_all = ["break_session", "custom"])]
    pub profile: Option<String>,

    /// Inicia pausa curta padrão de 5 minutos
    #[arg(long = "break", conflicts_with_all = ["profile", "custom"])]
    pub break_session: bool,

    /// Duração customizada em minutos
    #[arg(long, conflicts_with_all = ["profile", "break_session"], requires = "session_type")]
    pub custom: Option<u64>,

    /// Tipo da sessão customizada
    #[arg(long = "type", value_enum, requires = "custom")]
    pub session_type: Option<CustomSessionType>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum CustomSessionType {
    Focus,
    Break,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_start_profile() {
        let cli = Cli::try_parse_from(["pomo", "start", "--profile", "25-5"]).unwrap();
        let Some(Commands::Start(args)) = cli.command else {
            panic!("comando errado");
        };
        assert_eq!(args.profile.as_deref(), Some("25-5"));
    }

    #[test]
    fn parse_start_custom() {
        let cli =
            Cli::try_parse_from(["pomo", "start", "--custom", "45", "--type", "focus"]).unwrap();
        let Some(Commands::Start(args)) = cli.command else {
            panic!("comando errado");
        };
        assert_eq!(args.custom, Some(45));
        assert_eq!(args.session_type, Some(CustomSessionType::Focus));
    }

    #[test]
    fn rejeita_custom_sem_type() {
        assert!(Cli::try_parse_from(["pomo", "start", "--custom", "45"]).is_err());
    }
}
