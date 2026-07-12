use std::{
    fs,
    io::{ErrorKind, Write},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

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
    /// Stable identity used to make completion recovery idempotent.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Commit marker for the history side of completion.
    #[serde(default)]
    pub history_recorded: bool,
    /// Commit marker for the notification side of completion.
    #[serde(default)]
    pub notification_sent: bool,
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
            session_id: None,
            history_recorded: false,
            notification_sent: false,
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
        Ok(raw) => {
            let mut state: TimerState = serde_json::from_str(&raw).with_context(|| {
                format!("falha ao ler estado em {}", paths.state_file.display())
            })?;

            // Old state files have no identity or completion markers.  Generate
            // the identity here; the completion transaction persists it before
            // touching history, so a retry observes the same identity.
            if state.status == TimerStatus::Idle {
                state.session_id = None;
                state.history_recorded = false;
                state.notification_sent = false;
            } else if state.session_id.is_none() {
                state.session_id = Some(new_session_id());
            }
            Ok(state)
        }
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

    // Never truncate the live file.  A crash during write therefore leaves
    // either the previous complete JSON or the new complete JSON in place.
    let temp_path = temporary_state_path(paths);
    let write_result = (|| -> anyhow::Result<()> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temp_path)
            .with_context(|| format!("falha ao criar temporário {}", temp_path.display()))?;
        file.write_all(raw.as_bytes())
            .with_context(|| format!("falha ao gravar temporário {}", temp_path.display()))?;
        file.write_all(b"\n")
            .with_context(|| format!("falha ao finalizar temporário {}", temp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("falha ao sincronizar temporário {}", temp_path.display()))?;
        drop(file);

        fs::rename(&temp_path, &paths.state_file).with_context(|| {
            format!(
                "falha ao substituir {} atomicamente",
                paths.state_file.display()
            )
        })?;
        sync_directory(&paths.base_dir)?;
        Ok(())
    })();

    if let Err(error) = write_result {
        match fs::remove_file(&temp_path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(cleanup_error) => {
                return Err(error).context(format!(
                    "falha adicional ao limpar temporário {}: {cleanup_error}",
                    temp_path.display()
                ));
            }
        }
        return Err(error);
    }
    Ok(())
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temporary_state_path(paths: &StatePaths) -> PathBuf {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    paths.state_file.with_file_name(format!(
        "state.json.tmp.{}.{}",
        std::process::id(),
        nanos + counter as u128
    ))
}

pub(crate) fn new_session_id() -> String {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("session-{nanos}-{}-{counter}", std::process::id())
}

fn sync_directory(path: &std::path::Path) -> anyhow::Result<()> {
    fs::File::open(path)
        .with_context(|| format!("falha ao abrir diretório {} para sync", path.display()))?
        .sync_all()
        .with_context(|| format!("falha ao sincronizar diretório {}", path.display()))
}

/// Grava o estado apenas quando a transição realmente mudou algum campo.
pub fn write_state_if_changed(
    paths: &StatePaths,
    previous: &TimerState,
    next: &TimerState,
) -> anyhow::Result<bool> {
    write_state_if_changed_with(paths, previous, next, write_state)
}

fn write_state_if_changed_with(
    paths: &StatePaths,
    previous: &TimerState,
    next: &TimerState,
    writer: impl FnOnce(&StatePaths, &TimerState) -> anyhow::Result<()>,
) -> anyhow::Result<bool> {
    if previous == next {
        return Ok(false);
    }

    writer(paths, next)?;
    Ok(true)
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
            session_id: Some("session-test".to_string()),
            history_recorded: false,
            notification_sent: false,
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
    fn escrita_atomica_deixa_estado_completo_e_sem_temporario() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));
        let state = running_state();
        let writes = std::cell::Cell::new(0);

<<<<<<< HEAD
        write_state(&paths, &state).unwrap();

        let names = fs::read_dir(&paths.base_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["state.json"]);
        assert_eq!(read_state(&paths).unwrap(), state);
    }

    #[test]
    fn falha_no_rename_remove_temporario_e_preserva_alvo() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));
        paths.ensure_base_dir().unwrap();
        fs::create_dir(&paths.state_file).unwrap();

        assert!(write_state(&paths, &running_state()).is_err());
        assert!(paths.state_file.is_dir());
        assert_eq!(fs::read_dir(&paths.base_dir).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn estado_tem_permissao_de_usuario() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));
        write_state(&paths, &running_state()).unwrap();

        assert_eq!(
            fs::metadata(paths.state_file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn estado_legado_recebe_identidade_e_marcadores_padrao() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));
        paths.ensure_base_dir().unwrap();
        fs::write(
            &paths.state_file,
            r#"{"status":"running","session_type":"focus","label":"legado","duration_secs":60,"started_at":"2026-01-01T10:00:00-03:00","paused_remaining_secs":null}"#,
        )
        .unwrap();

        let state = read_state(&paths).unwrap();

        assert!(state.session_id.is_some());
        assert!(!state.history_recorded);
        assert!(!state.notification_sent);
    }

    #[test]
    fn nao_grava_estado_igual() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));
        let state = running_state();

        assert!(!write_state_if_changed(&paths, &state, &state).unwrap());
        assert!(!paths.state_file.exists());
=======
        assert!(
            !write_state_if_changed_with(&paths, &state, &state, |_, _| {
                writes.set(writes.get() + 1);
                Ok(())
            })
            .unwrap()
        );
        assert_eq!(writes.get(), 0);
>>>>>>> d2be24f (fix: keep tui errors scoped to refresh sources)
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
