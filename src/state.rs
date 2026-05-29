use std::{fs, io::ErrorKind, path::PathBuf};

use anyhow::Context;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimerStatus {
    Idle,
    Running,
    Paused,
    Finished,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionType {
    Focus,
    ShortBreak,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimerState {
    pub status: TimerStatus,
    pub session_type: SessionType,
    pub label: String,
    pub duration_secs: u64,
    pub started_at: Option<DateTime<Local>>,
    pub paused_remaining_secs: Option<u64>,
}

impl TimerState {
    pub fn idle() -> Self {
        Self {
            status: TimerStatus::Idle,
            session_type: SessionType::Focus,
            label: "Idle".to_string(),
            duration_secs: 0,
            started_at: None,
            paused_remaining_secs: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatePaths {
    pub base_dir: PathBuf,
    pub state_file: PathBuf,
    pub history_file: PathBuf,
    pub socket_file: PathBuf,
}

impl StatePaths {
    pub fn new() -> anyhow::Result<Self> {
        let state_root =
            dirs::state_dir().or_else(|| dirs::home_dir().map(|home| home.join(".local/state")));
        let state_root =
            state_root.context("não foi possível resolver o diretório local de estado")?;
        Ok(Self::from_base(state_root.join("omarchy-pomo")))
    }

    pub fn from_base(base_dir: impl Into<PathBuf>) -> Self {
        let base_dir = base_dir.into();
        Self {
            state_file: base_dir.join("state.json"),
            history_file: base_dir.join("history.jsonl"),
            socket_file: base_dir.join("pomo.sock"),
            base_dir,
        }
    }

    pub fn ensure_base_dir(&self) -> anyhow::Result<()> {
        fs::create_dir_all(&self.base_dir)
            .with_context(|| format!("falha ao criar {}", self.base_dir.display()))
    }
}

pub fn read_state(paths: &StatePaths) -> anyhow::Result<TimerState> {
    match fs::read_to_string(&paths.state_file) {
        Ok(raw) => serde_json::from_str(&raw)
            .with_context(|| format!("falha ao ler estado em {}", paths.state_file.display())),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(TimerState::idle()),
        Err(error) => Err(error).with_context(|| {
            format!(
                "falha ao abrir arquivo de estado {}",
                paths.state_file.display()
            )
        }),
    }
}

pub fn write_state(paths: &StatePaths, state: &TimerState) -> anyhow::Result<()> {
    paths.ensure_base_dir()?;
    let raw = serde_json::to_string_pretty(state).context("falha ao serializar estado")?;
    fs::write(&paths.state_file, raw)
        .with_context(|| format!("falha ao gravar {}", paths.state_file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn running_state() -> TimerState {
        TimerState {
            status: TimerStatus::Running,
            session_type: SessionType::Focus,
            label: "25/5 Focus".to_string(),
            duration_secs: 1_500,
            started_at: Some(Local::now()),
            paused_remaining_secs: None,
        }
    }

    #[test]
    fn serializa_e_deserializa_timer_state() {
        let state = running_state();

        let raw = serde_json::to_string(&state).unwrap();
        let parsed: TimerState = serde_json::from_str(&raw).unwrap();

        assert_eq!(parsed, state);
    }

    #[test]
    fn leitura_retorna_idle_quando_arquivo_nao_existe() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));

        let state = read_state(&paths).unwrap();

        assert_eq!(state, TimerState::idle());
    }

    #[test]
    fn escrita_seguida_de_leitura_preserva_estado() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));
        let state = running_state();

        write_state(&paths, &state).unwrap();
        let parsed = read_state(&paths).unwrap();

        assert_eq!(parsed, state);
    }

    #[test]
    fn caminhos_respeitam_base_injetavel() {
        let base = PathBuf::from("/tmp/omarchy-pomo-test");
        let paths = StatePaths::from_base(&base);

        assert_eq!(paths.base_dir, base);
        assert_eq!(
            paths.state_file,
            PathBuf::from("/tmp/omarchy-pomo-test/state.json")
        );
        assert_eq!(
            paths.history_file,
            PathBuf::from("/tmp/omarchy-pomo-test/history.jsonl")
        );
        assert_eq!(
            paths.socket_file,
            PathBuf::from("/tmp/omarchy-pomo-test/pomo.sock")
        );
    }
}
