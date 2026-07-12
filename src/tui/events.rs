use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    cli::{CustomSessionType, StartArgs},
    ipc::IpcRequest,
    state::{TimerState, TimerStatus},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiAction {
    Request(IpcRequest),
    BeginCustom,
    PushCustomDigit(char),
    PopCustomDigit,
    SetCustomType(CustomSessionType),
    SubmitCustom,
    CancelCustom,
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
        KeyCode::Char('4') => TuiAction::BeginCustom,
        KeyCode::Char('p') => match state.map(|state| &state.status) {
            Some(TimerStatus::Paused) => TuiAction::Request(IpcRequest::Resume),
            _ => TuiAction::Request(IpcRequest::Pause),
        },
        KeyCode::Char('s') => TuiAction::Request(IpcRequest::Stop),
        _ => TuiAction::None,
    }
}

pub fn action_for_custom_key(key: KeyEvent) -> TuiAction {
    match key.code {
        KeyCode::Esc => TuiAction::CancelCustom,
        KeyCode::Enter => TuiAction::SubmitCustom,
        KeyCode::Backspace => TuiAction::PopCustomDigit,
        KeyCode::Char('f') | KeyCode::Char('F') => {
            TuiAction::SetCustomType(CustomSessionType::Focus)
        }
        KeyCode::Char('b') | KeyCode::Char('B') => {
            TuiAction::SetCustomType(CustomSessionType::Break)
        }
        KeyCode::Char(ch) if ch.is_ascii_digit() => TuiAction::PushCustomDigit(ch),
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
            category: crate::state::SessionCategory::Focus,
            label: "teste".to_string(),
            duration_secs: 60,
            started_at: None,
            paused_remaining_secs: Some(30),
            session_id: Some("session-test".to_string()),
            history_recorded: false,
            notification_sent: false,
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

    #[test]
    fn tecla_4_inicia_fluxo_custom() {
        assert_eq!(action_for_key(key('4'), None), TuiAction::BeginCustom);
    }

    #[test]
    fn input_custom_aceita_digitos_tipo_e_enter() {
        assert_eq!(
            action_for_custom_key(key('7')),
            TuiAction::PushCustomDigit('7')
        );
        assert_eq!(
            action_for_custom_key(key('f')),
            TuiAction::SetCustomType(CustomSessionType::Focus)
        );
        assert_eq!(
            action_for_custom_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            TuiAction::SubmitCustom
        );
    }

    #[test]
    fn input_custom_ignora_texto_invalido_e_negativo() {
        assert_eq!(action_for_custom_key(key('x')), TuiAction::None);
        assert_eq!(action_for_custom_key(key('-')), TuiAction::None);
    }
}
