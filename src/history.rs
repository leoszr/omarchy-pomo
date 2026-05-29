use std::{fs, io::ErrorKind};

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
    pub date: NaiveDate,
    #[serde(rename = "type")]
    pub session_type: HistorySessionType,
    pub label: String,
    pub duration_secs: u64,
    pub completed: bool,
    pub finished_at: DateTime<Local>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DailySummary {
    pub date: NaiveDate,
    pub focus_sessions: u64,
    pub focused_secs: u64,
    pub break_sessions: u64,
}

impl HistoryEntry {
    pub fn completed_from_state(state: &TimerState, finished_at: DateTime<Local>) -> Self {
        Self {
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
    paths.ensure_base_dir()?;
    let line = serde_json::to_string(entry).context("falha ao serializar histórico")?;
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.history_file)
        .with_context(|| format!("falha ao abrir {}", paths.history_file.display()))?
        .write_all_line(&line)
        .with_context(|| format!("falha ao gravar {}", paths.history_file.display()))
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
    raw.lines()
        .filter_map(|line| serde_json::from_str::<HistoryEntry>(line).ok())
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

trait WriteLine {
    fn write_all_line(&mut self, line: &str) -> std::io::Result<()>;
}

impl<T: std::io::Write> WriteLine for T {
    fn write_all_line(&mut self, line: &str) -> std::io::Result<()> {
        self.write_all(line.as_bytes())?;
        self.write_all(b"\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::TimerStatus;

    fn entry(session_type: HistorySessionType, date: NaiveDate, secs: u64) -> HistoryEntry {
        HistoryEntry {
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
        };

        let entry = HistoryEntry::completed_from_state(&state, now);

        assert_eq!(entry.session_type, HistorySessionType::Focus);
        assert!(entry.completed);
    }
}
