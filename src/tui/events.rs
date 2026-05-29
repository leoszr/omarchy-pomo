use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    cli::StartArgs,
    ipc::IpcRequest,
    state::{TimerState, TimerStatus},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiAction {
    Request(IpcRequest),
    Quit,
    None,
}

pub fn action_for_key(key: KeyEvent, state: Option<&TimerState>) -> TuiAction {
    match key.code {
        KeyCode::Char('q') => TuiAction::Quit,
        KeyCode::Char('1') => TuiAction::Request(IpcRequest::Start {
            args: profile_args("25-5"),
        }),
        KeyCode::Char('2') => TuiAction::Request(IpcRequest::Start {
            args: profile_args("30-10"),
        }),
        KeyCode::Char('3') => TuiAction::Request(IpcRequest::Start { args: break_args() }),
        KeyCode::Char('p') => match state.map(|state| &state.status) {
            Some(TimerStatus::Paused) => TuiAction::Request(IpcRequest::Resume),
            _ => TuiAction::Request(IpcRequest::Pause),
        },
        KeyCode::Char('s') => TuiAction::Request(IpcRequest::Stop),
        _ => TuiAction::None,
    }
}

fn profile_args(profile: &str) -> StartArgs {
    StartArgs {
        profile: Some(profile.to_string()),
        break_session: false,
        custom: None,
        session_type: None,
    }
}

fn break_args() -> StartArgs {
    StartArgs {
        profile: None,
        break_session: true,
        custom: None,
        session_type: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(ch: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)
    }

    #[test]
    fn tecla_1_gera_start_25() {
        let TuiAction::Request(IpcRequest::Start { args }) = action_for_key(key('1'), None) else {
            panic!("ação errada");
        };

        assert_eq!(args.profile.as_deref(), Some("25-5"));
    }

    #[test]
    fn tecla_p_em_paused_gera_resume() {
        let state = TimerState {
            status: TimerStatus::Paused,
            session_type: crate::state::SessionType::Focus,
            label: "teste".to_string(),
            duration_secs: 60,
            started_at: None,
            paused_remaining_secs: Some(30),
        };

        assert_eq!(
            action_for_key(key('p'), Some(&state)),
            TuiAction::Request(IpcRequest::Resume)
        );
    }

    #[test]
    fn q_nao_envia_stop() {
        assert_eq!(action_for_key(key('q'), None), TuiAction::Quit);
    }
}
