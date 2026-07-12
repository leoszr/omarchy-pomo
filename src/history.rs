use std::{
    collections::HashSet,
    fs,
    io::{ErrorKind, Write},
};

use anyhow::Context;
use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::state::{SessionType, StatePaths, TimerState};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistorySessionType {
    Focus,
    Break,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// `None` is accepted for entries written before session identities existed.
    #[serde(default)]
    pub session_id: Option<String>,
    pub date: NaiveDate,
    #[serde(rename = "type")]
    pub session_type: HistorySessionType,
    pub label: String,
    pub duration_secs: u64,
    pub completed: bool,
    pub finished_at: DateTime<Local>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DailySummary {
    pub date: NaiveDate,
    pub focus_sessions: u64,
    pub focused_secs: u64,
    pub break_sessions: u64,
}

impl HistoryEntry {
    pub fn completed_from_state(state: &TimerState, finished_at: DateTime<Local>) -> Self {
        Self {
            session_id: state.session_id.clone(),
            date: finished_at.date_naive(),
            session_type: history_session_type(state),
            label: state.label.clone(),
            duration_secs: state.duration_secs,
            completed: true,
            finished_at,
        }
    }
}

pub fn append_entry(paths: &StatePaths, entry: &HistoryEntry) -> anyhow::Result<()> {
    append_entry_if_absent(paths, entry).map(|_| ())
}

/// Appends only when this session is not already in the history.
pub fn append_entry_if_absent(paths: &StatePaths, entry: &HistoryEntry) -> anyhow::Result<bool> {
    paths.ensure_base_dir()?;
    let line = serde_json::to_string(entry).context("falha ao serializar histórico")?;
    if read_entries(paths)?
        .iter()
        .any(|existing| same_session(existing, entry))
    {
        return Ok(false);
    }

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.history_file)
        .with_context(|| format!("falha ao abrir {}", paths.history_file.display()))?;
    file.write_all(line.as_bytes())
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_data())
        .with_context(|| format!("falha ao gravar {}", paths.history_file.display()))?;
    Ok(true)
}

pub fn read_entries(paths: &StatePaths) -> anyhow::Result<Vec<HistoryEntry>> {
    let raw = match fs::read_to_string(&paths.history_file) {
        Ok(raw) => raw,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("falha ao abrir {}", paths.history_file.display()))
        }
    };

    Ok(parse_jsonl_lossy(&raw))
}

pub fn parse_jsonl_lossy(raw: &str) -> Vec<HistoryEntry> {
    let mut seen = HashSet::new();
    raw.lines()
        .filter_map(|line| serde_json::from_str::<HistoryEntry>(line).ok())
        .filter(|entry| seen.insert(session_key(entry)))
        .collect()
}

pub fn summarize_day(entries: &[HistoryEntry], date: NaiveDate) -> DailySummary {
    let mut summary = DailySummary {
        date,
        ..DailySummary::default()
    };

    for entry in entries
        .iter()
        .filter(|entry| entry.completed && entry.date == date)
    {
        match entry.session_type {
            HistorySessionType::Focus => {
                summary.focus_sessions += 1;
                summary.focused_secs += entry.duration_secs;
            }
            HistorySessionType::Break => summary.break_sessions += 1,
        }
    }

    summary
}

fn history_session_type(state: &TimerState) -> HistorySessionType {
    match state.session_type {
        SessionType::ShortBreak => HistorySessionType::Break,
        SessionType::Focus => HistorySessionType::Focus,
        SessionType::Custom if state.label.to_ascii_lowercase().contains("break") => {
            HistorySessionType::Break
        }
        SessionType::Custom => HistorySessionType::Focus,
    }
}

fn same_session(left: &HistoryEntry, right: &HistoryEntry) -> bool {
    match (left.session_id.as_deref(), right.session_id.as_deref()) {
        (Some(left_id), Some(right_id)) => left_id == right_id,
        (None, None) => legacy_key(left) == legacy_key(right),
        // There is no safe identity bridge between formats.  Matching on
        // date/type/label/duration would discard a legitimate new session
        // after an older session with the same preset.
        _ => false,
    }
}

fn session_key(entry: &HistoryEntry) -> String {
    if let Some(session_id) = entry.session_id.as_deref().filter(|id| !id.is_empty()) {
        return format!("id:{session_id}");
    }

    legacy_key(entry)
}

fn legacy_key(entry: &HistoryEntry) -> String {
    // Keep the timestamp for legacy-vs-legacy deduplication: two old sessions
    // may legitimately have the same type, label and duration.
    format!(
        "legacy:{}:{}:{}:{}:{}",
        entry.date,
        match entry.session_type {
            HistorySessionType::Focus => "focus",
            HistorySessionType::Break => "break",
        },
        entry.label,
        entry.duration_secs,
        entry.finished_at
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::TimerStatus;

    fn entry(session_type: HistorySessionType, date: NaiveDate, secs: u64) -> HistoryEntry {
        HistoryEntry {
            session_id: None,
            date,
            session_type,
            label: "teste".to_string(),
            duration_secs: secs,
            completed: true,
            finished_at: Local::now(),
        }
    }

    #[test]
    fn parse_jsonl_valido() {
        let now = Local::now();
        let raw = format!(
            "{}\n",
            serde_json::to_string(&HistoryEntry {
                session_id: Some("session-test".to_string()),
                date: now.date_naive(),
                session_type: HistorySessionType::Focus,
                label: "25/5 Focus".to_string(),
                duration_secs: 1_500,
                completed: true,
                finished_at: now,
            })
            .unwrap()
        );

        let entries = parse_jsonl_lossy(&raw);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].duration_secs, 1_500);
    }

    #[test]
    fn linhas_invalidas_sao_ignoradas_no_resumo() {
        let now = Local::now();
        let valid =
            serde_json::to_string(&entry(HistorySessionType::Focus, now.date_naive(), 60)).unwrap();
        let entries = parse_jsonl_lossy(&format!("nao-json\n{valid}\n"));

        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn resumo_do_dia_soma_foco_e_pausas() {
        let today = Local::now().date_naive();
        let entries = vec![
            entry(HistorySessionType::Focus, today, 1_500),
            entry(HistorySessionType::Focus, today, 1_800),
            entry(HistorySessionType::Break, today, 300),
        ];

        let summary = summarize_day(&entries, today);

        assert_eq!(summary.focus_sessions, 2);
        assert_eq!(summary.focused_secs, 3_300);
        assert_eq!(summary.break_sessions, 1);
    }

    #[test]
    fn cria_entry_de_sessao_concluida() {
        let now = Local::now();
        let state = TimerState {
            status: TimerStatus::Finished,
            session_type: SessionType::Focus,
            label: "25/5 Focus".to_string(),
            duration_secs: 1_500,
            started_at: None,
            paused_remaining_secs: Some(0),
            session_id: Some("session-test".to_string()),
            history_recorded: false,
            notification_sent: false,
        };

        let entry = HistoryEntry::completed_from_state(&state, now);

        assert_eq!(entry.session_type, HistorySessionType::Focus);
        assert_eq!(entry.session_id.as_deref(), Some("session-test"));
        assert!(entry.completed);
    }

    #[test]
    fn deduplica_ids_e_registros_legados() {
        let now = Local::now();
        let with_id = HistoryEntry {
            session_id: Some("same".to_string()),
            ..entry(HistorySessionType::Focus, now.date_naive(), 60)
        };
        let legacy = entry(HistorySessionType::Break, now.date_naive(), 30);
        let raw = format!(
            "{}\n{}\n{}\n",
            serde_json::to_string(&with_id).unwrap(),
            serde_json::to_string(&with_id).unwrap(),
            serde_json::to_string(&legacy).unwrap()
        );

        assert_eq!(parse_jsonl_lossy(&raw).len(), 2);
    }

    #[test]
    fn append_retry_nao_cria_linha_duplicada() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));
        let entry = entry(HistorySessionType::Focus, Local::now().date_naive(), 60);

        assert!(append_entry_if_absent(&paths, &entry).unwrap());
        assert!(!append_entry_if_absent(&paths, &entry).unwrap());
        assert_eq!(read_entries(&paths).unwrap().len(), 1);
    }

    #[test]
    fn sessao_nova_nao_colide_com_legada_do_mesmo_preset() {
        let dir = tempfile::tempdir().unwrap();
        let paths = StatePaths::from_base(dir.path().join("omarchy-pomo"));
        let now = Local::now();
        let legacy = HistoryEntry {
            finished_at: now - chrono::Duration::hours(1),
            ..entry(HistorySessionType::Focus, now.date_naive(), 1_500)
        };
        let current = HistoryEntry {
            session_id: Some("new-session".to_string()),
            finished_at: now,
            ..legacy.clone()
        };

        append_entry(&paths, &legacy).unwrap();
        assert!(append_entry_if_absent(&paths, &current).unwrap());

        assert_eq!(read_entries(&paths).unwrap().len(), 2);
    }
}
