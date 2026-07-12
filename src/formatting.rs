use crate::state::{SessionCategory, SessionType, TimerStatus};

pub(crate) fn duration(secs: u64) -> String {
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

pub(crate) fn status(status: &TimerStatus) -> &'static str {
    match status {
        TimerStatus::Idle => "idle",
        TimerStatus::Running => "running",
        TimerStatus::Paused => "paused",
        TimerStatus::Finished => "finished",
    }
}

pub(crate) fn session_type(session_type: &SessionType) -> &'static str {
    match session_type {
        SessionType::Focus => "focus",
        SessionType::ShortBreak => "short_break",
        SessionType::Custom => "custom",
    }
}

pub(crate) fn category(category: SessionCategory) -> &'static str {
    match category {
        SessionCategory::Focus => "focus",
        SessionCategory::Break => "break",
    }
}
