use crate::{
    cli::{CustomSessionType, StartArgs},
    history::DailySummary,
    ipc::IpcResponse,
    state::TimerState,
    timer,
};

#[derive(Debug, Clone, Default)]
pub struct TuiApp {
    pub state: Option<TimerState>,
    pub summary: Option<DailySummary>,
    pub error: Option<String>,
    pub should_quit: bool,
    pub custom_input: Option<CustomInput>,
    status_error: Option<String>,
    history_error: Option<String>,
    action_error: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ErrorSource {
    Status,
    History,
    Action,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CustomInput {
    pub minutes: String,
    pub session_type: Option<CustomSessionType>,
}

impl TuiApp {
    pub fn apply_response(&mut self, response: IpcResponse) {
        self.apply_response_from(ErrorSource::Action, response);
    }

    pub(super) fn apply_response_from(&mut self, source: ErrorSource, response: IpcResponse) {
        match response {
            IpcResponse::State { state } => {
                self.state = Some(state);
                self.clear_error_source(source);
            }
            IpcResponse::History { summary } => {
                self.summary = Some(summary);
                self.clear_error_source(source);
            }
            IpcResponse::Error { message } => self.set_error_source(source, message),
        }
    }

    pub fn set_error(&mut self, error: impl Into<String>) {
        self.set_error_source(ErrorSource::Action, error);
    }

    pub(super) fn set_error_source(&mut self, source: ErrorSource, error: impl Into<String>) {
        let error = Some(error.into());
        match source {
            ErrorSource::Status => self.status_error = error,
            ErrorSource::History => self.history_error = error,
            ErrorSource::Action => self.action_error = error,
        }
        self.refresh_error();
    }

    fn clear_error_source(&mut self, source: ErrorSource) {
        match source {
            ErrorSource::Status => self.status_error = None,
            ErrorSource::History => self.history_error = None,
            ErrorSource::Action => self.action_error = None,
        }
        self.refresh_error();
    }

    fn refresh_error(&mut self) {
        let errors = [
            self.action_error
                .as_deref()
                .map(|error| format!("ação: {error}")),
            self.status_error
                .as_deref()
                .map(|error| format!("status: {error}")),
            self.history_error
                .as_deref()
                .map(|error| format!("histórico: {error}")),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

        self.error = match errors.as_slice() {
            [] => None,
            [error] => Some(
                error
                    .split_once(": ")
                    .map_or_else(|| error.clone(), |(_, message)| message.to_string()),
            ),
            _ => Some(errors.join(" | ")),
        };
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

    pub fn begin_custom_input(&mut self) {
        self.clear_error_source(ErrorSource::Action);
        self.custom_input = Some(CustomInput::default());
    }

    pub fn push_custom_digit(&mut self, digit: char) {
        if let Some(input) = &mut self.custom_input {
            if input.minutes.len() < 4 {
                input.minutes.push(digit);
            }
        }
    }

    pub fn pop_custom_digit(&mut self) {
        if let Some(input) = &mut self.custom_input {
            input.minutes.pop();
        }
    }

    pub fn set_custom_type(&mut self, session_type: CustomSessionType) {
        if let Some(input) = &mut self.custom_input {
            input.session_type = Some(session_type);
        }
    }

    pub fn cancel_custom_input(&mut self) {
        self.custom_input = None;
        self.clear_error_source(ErrorSource::Action);
    }

    pub fn custom_start_args(&self) -> Result<StartArgs, String> {
        let input = self
            .custom_input
            .as_ref()
            .ok_or_else(|| "input customizado não iniciado".to_string())?;
        let minutes: u64 = input
            .minutes
            .parse()
            .map_err(|_| "digite minutos válidos".to_string())?;
        crate::timer::duration_secs_from_minutes(minutes).map_err(|error| format!("{error:#}"))?;
        let session_type = input
            .session_type
            .ok_or_else(|| "escolha f para foco ou b para break".to_string())?;
        Ok(StartArgs {
            profile: None,
            break_session: false,
            custom: Some(minutes),
            session_type: Some(session_type),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cli::CustomSessionType, ipc::IpcResponse, state::SessionType};

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

    #[test]
    fn resposta_bem_sucedida_nao_apaga_erro_do_mesmo_ciclo() {
        let mut app = TuiApp::default();
        app.apply_response_from(
            ErrorSource::History,
            IpcResponse::Error {
                message: "histórico indisponível".to_string(),
            },
        );
        app.apply_response_from(
            ErrorSource::Status,
            IpcResponse::State {
                state: crate::state::TimerState::idle(),
            },
        );

        assert_eq!(app.error.as_deref(), Some("histórico indisponível"));

        app.apply_response_from(
            ErrorSource::History,
            IpcResponse::History {
                summary: DailySummary::default(),
            },
        );
        assert!(app.error.is_none());
    }

    #[test]
    fn custom_input_valido_gera_start_args() {
        let mut app = TuiApp::default();
        app.begin_custom_input();
        app.push_custom_digit('4');
        app.push_custom_digit('5');
        app.set_custom_type(CustomSessionType::Focus);

        let args = app.custom_start_args().unwrap();

        assert_eq!(args.custom, Some(45));
        assert_eq!(args.session_type, Some(CustomSessionType::Focus));
    }

    #[test]
    fn custom_input_rejeita_zero() {
        let mut app = TuiApp::default();
        app.begin_custom_input();
        app.push_custom_digit('0');
        app.set_custom_type(CustomSessionType::Break);

        assert!(app
            .custom_start_args()
            .unwrap_err()
            .contains("maior que zero"));
    }

    #[test]
    fn custom_input_exige_tipo() {
        let mut app = TuiApp::default();
        app.begin_custom_input();
        app.push_custom_digit('5');

        assert!(app.custom_start_args().unwrap_err().contains("escolha"));
    }
}
