use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::state::TimerState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
}

pub trait CompletionNotifier {
    fn notify_completed(&mut self, state: &TimerState) -> anyhow::Result<()>;
}

#[derive(Debug, Default)]
pub struct ExternalNotifier;

impl CompletionNotifier for ExternalNotifier {
    fn notify_completed(&mut self, state: &TimerState) -> anyhow::Result<()> {
        notify_completed_best_effort(state);
        Ok(())
    }
}

pub fn notify_completed_best_effort(state: &TimerState) {
    run_best_effort(&notify_send_command(state));

    if let Some(sound_file) = sound_file_path().filter(|path| path.exists()) {
        play_sound_best_effort(&sound_file);
    }
}

pub fn notify_send_command(state: &TimerState) -> CommandSpec {
    CommandSpec {
        program: "notify-send".to_string(),
        args: vec![
            "Pomodoro finalizado".to_string(),
            format!("Sessão concluída: {}", state.label),
        ],
    }
}

pub fn paplay_command(sound_file: &Path) -> CommandSpec {
    CommandSpec {
        program: "paplay".to_string(),
        args: vec![sound_file.display().to_string()],
    }
}

pub fn mpv_command(sound_file: &Path) -> CommandSpec {
    CommandSpec {
        program: "mpv".to_string(),
        args: vec!["--no-video".to_string(), sound_file.display().to_string()],
    }
}

pub fn sound_file_path() -> Option<PathBuf> {
    let config_root =
        dirs::config_dir().or_else(|| dirs::home_dir().map(|home| home.join(".config")))?;
    Some(config_root.join("omarchy-pomo/done.ogg"))
}

pub fn play_sound_best_effort(sound_file: &Path) {
    if run_best_effort(&paplay_command(sound_file)) {
        return;
    }
    run_best_effort(&mpv_command(sound_file));
}

fn run_best_effort(spec: &CommandSpec) -> bool {
    Command::new(&spec.program)
        .args(&spec.args)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{SessionType, TimerStatus};

    fn finished_state() -> TimerState {
        TimerState {
            status: TimerStatus::Finished,
            session_type: SessionType::Focus,
            label: "25/5 Focus".to_string(),
            duration_secs: 1_500,
            started_at: None,
            paused_remaining_secs: Some(0),
            session_id: Some("session-test".to_string()),
            history_recorded: false,
            notification_sent: false,
        }
    }

    #[test]
    fn constroi_comando_notify_send() {
        let command = notify_send_command(&finished_state());

        assert_eq!(command.program, "notify-send");
        assert_eq!(command.args[0], "Pomodoro finalizado");
        assert!(command.args[1].contains("25/5 Focus"));
    }

    #[test]
    fn constroi_comandos_de_som() {
        let path = PathBuf::from("/tmp/done.ogg");

        assert_eq!(paplay_command(&path).args, vec!["/tmp/done.ogg"]);
        assert_eq!(mpv_command(&path).args, vec!["--no-video", "/tmp/done.ogg"]);
    }

    #[test]
    fn ausencia_de_arquivo_de_som_nao_gera_erro() {
        notify_completed_best_effort(&finished_state());
    }
}
