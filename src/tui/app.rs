use crate::{history::DailySummary, ipc::IpcResponse, state::TimerState, timer};

#[derive(Debug, Clone, Default)]
pub struct TuiApp {
    pub state: Option<TimerState>,
    pub summary: Option<DailySummary>,
    pub error: Option<String>,
    pub should_quit: bool,
}

impl TuiApp {
    pub fn apply_response(&mut self, response: IpcResponse) {
        self.error = None;
        match response {
            IpcResponse::State { state } => self.state = Some(state),
            IpcResponse::History { summary } => self.summary = Some(summary),
            IpcResponse::Error { message } => self.error = Some(message),
        }
    }

    pub fn set_error(&mut self, error: impl Into<String>) {
        self.error = Some(error.into());
    }

    pub fn remaining_secs(&self) -> u64 {
        self.state
            .as_ref()
            .map(|state| timer::remaining_secs_at(state, chrono::Local::now()))
            .unwrap_or(0)
    }

    pub fn progress_ratio(&self) -> f64 {
        let Some(state) = &self.state else {
            return 0.0;
        };
        if state.duration_secs == 0 {
            return 0.0;
        }
        let elapsed = state.duration_secs.saturating_sub(self.remaining_secs());
        elapsed as f64 / state.duration_secs as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ipc::IpcResponse, state::SessionType};

    #[test]
    fn interpreta_response_state() {
        let mut app = TuiApp::default();
        let state = crate::timer::start_session(
            SessionType::Focus,
            "Foco".to_string(),
            60,
            chrono::Local::now(),
        );

        app.apply_response(IpcResponse::State {
            state: state.clone(),
        });

        assert_eq!(app.state, Some(state));
        assert!(app.error.is_none());
    }

    #[test]
    fn interpreta_response_error() {
        let mut app = TuiApp::default();

        app.apply_response(IpcResponse::Error {
            message: "daemon caiu".to_string(),
        });

        assert_eq!(app.error.as_deref(), Some("daemon caiu"));
    }
}
