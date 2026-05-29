use serde::{Deserialize, Serialize};

use crate::{
    state::{SessionType, TimerState, TimerStatus},
    timer,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaybarOutput {
    pub text: String,
    pub tooltip: String,
    #[serde(rename = "class")]
    pub class_name: String,
}

pub fn from_state(state: &TimerState, now: chrono::DateTime<chrono::Local>) -> WaybarOutput {
    let remaining = timer::remaining_secs_at(state, now);
    let remaining_text = format_duration(remaining);
    let class_name = class_for(state).to_string();
    let text = match state.status {
        TimerStatus::Idle => "󰔟".to_string(),
        TimerStatus::Running if is_break(state) => format!("☕ {remaining_text}"),
        TimerStatus::Running => format!("󰔟 {remaining_text}"),
        TimerStatus::Paused => format!("󰏤 {remaining_text}"),
        TimerStatus::Finished => " pronto".to_string(),
    };

    WaybarOutput {
        text,
        tooltip: format!("Pomodoro: {}", state.label),
        class_name,
    }
}

pub fn error(message: &str) -> WaybarOutput {
    WaybarOutput {
        text: " daemon".to_string(),
        tooltip: message.to_string(),
        class_name: "error".to_string(),
    }
}

pub fn to_json(output: &WaybarOutput) -> anyhow::Result<String> {
    Ok(serde_json::to_string(output)?)
}

fn class_for(state: &TimerState) -> &'static str {
    match state.status {
        TimerStatus::Idle => "idle",
        TimerStatus::Paused => "paused",
        TimerStatus::Finished => "finished",
        TimerStatus::Running if is_break(state) => "break",
        TimerStatus::Running => "running",
    }
}

fn is_break(state: &TimerState) -> bool {
    state.session_type == SessionType::ShortBreak
        || (state.session_type == SessionType::Custom
            && state.label.to_ascii_lowercase().contains("break"))
}

fn format_duration(secs: u64) -> String {
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::SessionType;

    #[test]
    fn json_gerado_e_valido() {
        let output = error("daemon indisponível");
        let raw = to_json(&output).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();

        assert_eq!(parsed["class"], "error");
    }

    #[test]
    fn classe_running_para_foco() {
        let now = chrono::Local::now();
        let state = crate::timer::start_session(SessionType::Focus, "Foco".to_string(), 60, now);

        assert_eq!(from_state(&state, now).class_name, "running");
    }

    #[test]
    fn classe_break_para_pausa() {
        let now = chrono::Local::now();
        let state = crate::timer::start_session(
            SessionType::ShortBreak,
            "Short Break".to_string(),
            60,
            now,
        );

        assert_eq!(from_state(&state, now).class_name, "break");
    }

    #[test]
    fn formata_mm_ss() {
        let now = chrono::Local::now();
        let state = crate::timer::start_session(SessionType::Focus, "Foco".to_string(), 65, now);

        assert_eq!(from_state(&state, now).text, "󰔟 01:05");
    }
}
