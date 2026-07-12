use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

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
    /// Inicia daemon Pomodoro via Unix Socket
    Daemon,
    /// Mostra o status do timer
    Status(StatusArgs),
    /// Pausa a sessão atual
    Pause,
    /// Retoma a sessão pausada
    Resume,
    /// Para a sessão atual e volta para idle
    Stop,
    /// Mostra resumo do histórico de hoje
    History,
    /// Abre TUI para controlar o daemon
    Tui,
    /// Gerencia tarefas
    Task {
        #[arg(default_value = "list")]
        action: String,
    },
}

#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Imprime JSON compatível com Waybar
    #[arg(long)]
    pub waybar: bool,
}

#[derive(Args, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartArgs {
    /// Perfil de foco: 25-5 ou 30-10
    #[arg(long, value_parser = ["25-5", "30-10"], conflicts_with_all = ["break_session", "custom"])]
    pub profile: Option<String>,

    /// Inicia pausa curta padrão de 5 minutos
    #[arg(long = "break", conflicts_with_all = ["profile", "custom"])]
    pub break_session: bool,

    /// Duração customizada em minutos
    #[arg(
        long,
        value_parser = parse_custom_minutes,
        conflicts_with_all = ["profile", "break_session"],
        requires = "session_type"
    )]
    pub custom: Option<u64>,

    /// Tipo da sessão customizada
    #[arg(long = "type", value_enum, requires = "custom")]
    pub session_type: Option<CustomSessionType>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum CustomSessionType {
    Focus,
    Break,
}

/// Valida o limite também no parser da CLI. O domínio repete a validação porque
/// `StartArgs` também chega por desserialização no protocolo IPC.
pub fn parse_custom_minutes(value: &str) -> Result<u64, String> {
    let minutes = value
        .parse::<u64>()
        .map_err(|_| "minutos customizados devem ser um número inteiro".to_string())?;
    if minutes == 0 {
        return Err("tempo customizado deve ser maior que zero".to_string());
    }
    if minutes > crate::timer::MAX_CUSTOM_MINUTES {
        return Err(format!(
            "tempo customizado não pode exceder {} minutos",
            crate::timer::MAX_CUSTOM_MINUTES
        ));
    }
    minutes
        .checked_mul(60)
        .ok_or_else(|| "duração customizada excede o limite suportado".to_string())?;
    Ok(minutes)
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

    #[test]
    fn parse_status_waybar() {
        let cli = Cli::try_parse_from(["pomo", "status", "--waybar"]).unwrap();
        let Some(Commands::Status(args)) = cli.command else {
            panic!("comando errado");
        };
        assert!(args.waybar);
    }

    #[test]
    fn rejeita_custom_fora_do_limite() {
        assert!(
            Cli::try_parse_from(["pomo", "start", "--custom", "1441", "--type", "focus"]).is_err()
        );
    }

    #[test]
    fn rejeita_custom_zero_na_cli() {
        assert!(
            Cli::try_parse_from(["pomo", "start", "--custom", "0", "--type", "focus"]).is_err()
        );
    }
}
