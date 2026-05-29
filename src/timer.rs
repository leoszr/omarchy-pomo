use chrono::{DateTime, Local};

use crate::state::{SessionType, TimerState, TimerStatus};

pub fn start_focus_25(now: DateTime<Local>) -> TimerState {
    TimerState {
        status: TimerStatus::Running,
        session_type: SessionType::Focus,
        label: "25/5 Focus".to_string(),
        duration_secs: 25 * 60,
        started_at: Some(now),
        paused_remaining_secs: None,
    }
}

pub fn remaining_secs_at(state: &TimerState, now: DateTime<Local>) -> u64 {
    match state.status {
        TimerStatus::Idle | TimerStatus::Finished => 0,
        TimerStatus::Paused => state.paused_remaining_secs.unwrap_or(0),
        TimerStatus::Running => {
            let Some(started_at) = state.started_at else {
                return 0;
            };
            let elapsed = now.signed_duration_since(started_at).num_seconds().max(0) as u64;
            state.duration_secs.saturating_sub(elapsed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::SessionType;
    use chrono::Duration;

    #[test]
    fn restante_de_estado_running_usa_timestamp() {
        let now = Local::now();
        let state = TimerState {
            status: TimerStatus::Running,
            session_type: SessionType::Focus,
            label: "teste".to_string(),
            duration_secs: 60,
            started_at: Some(now - Duration::seconds(10)),
            paused_remaining_secs: None,
        };

        assert_eq!(remaining_secs_at(&state, now), 50);
    }

    #[test]
    fn restante_nao_fica_negativo() {
        let now = Local::now();
        let state = TimerState {
            status: TimerStatus::Running,
            session_type: SessionType::Focus,
            label: "teste".to_string(),
            duration_secs: 60,
            started_at: Some(now - Duration::seconds(120)),
            paused_remaining_secs: None,
        };

        assert_eq!(remaining_secs_at(&state, now), 0);
    }
}
