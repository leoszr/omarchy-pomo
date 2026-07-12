use anyhow::{bail, ensure};
use chrono::{DateTime, Local};

use crate::{
    cli::{CustomSessionType, StartArgs},
    state::{SessionCategory, SessionType, TimerState, TimerStatus},
};

/// Limite deliberado para evitar durações absurdas e overflow na conversão
/// de minutos para segundos. Sessões maiores não são úteis para este timer.
pub const MAX_CUSTOM_MINUTES: u64 = 24 * 60;

pub fn duration_secs_from_minutes(minutes: u64) -> anyhow::Result<u64> {
    ensure!(minutes > 0, "tempo customizado deve ser maior que zero");
    ensure!(
        minutes <= MAX_CUSTOM_MINUTES,
        "tempo customizado não pode exceder {MAX_CUSTOM_MINUTES} minutos"
    );
    minutes
        .checked_mul(60)
        .ok_or_else(|| anyhow::anyhow!("duração customizada excede o limite suportado"))
}

pub fn start_from_args(args: &StartArgs, now: DateTime<Local>) -> anyhow::Result<TimerState> {
    if args.break_session {
        return Ok(start_session(
            SessionType::ShortBreak,
            "Short Break".to_string(),
            5 * 60,
            now,
        ));
    }

    if let Some(minutes) = args.custom {
        let category = match args.session_type {
            Some(CustomSessionType::Focus) => SessionCategory::Focus,
            Some(CustomSessionType::Break) => SessionCategory::Break,
            None => bail!("--custom exige --type <focus|break>"),
        };
        let category_name = match category {
            SessionCategory::Focus => "Focus",
            SessionCategory::Break => "Break",
        };
        return Ok(start_custom_session(
            category,
            format!("Custom {category_name} ({minutes} min)"),
            duration_secs_from_minutes(minutes)?,
            now,
        ));
    }

    match args.profile.as_deref().unwrap_or("25-5") {
        "25-5" => Ok(start_session(
            SessionType::Focus,
            "25/5 Focus".to_string(),
            25 * 60,
            now,
        )),
        "30-10" => Ok(start_session(
            SessionType::Focus,
            "30/10 Focus".to_string(),
            30 * 60,
            now,
        )),
        unknown => bail!("perfil inválido: {unknown}"),
    }
}

pub fn start_session(
    session_type: SessionType,
    label: String,
    duration_secs: u64,
    now: DateTime<Local>,
) -> TimerState {
    let category = match session_type {
        SessionType::ShortBreak => SessionCategory::Break,
        SessionType::Focus | SessionType::Custom => SessionCategory::Focus,
    };
    start_session_with_category(session_type, category, label, duration_secs, now)
}

pub fn start_custom_session(
    category: SessionCategory,
    label: String,
    duration_secs: u64,
    now: DateTime<Local>,
) -> TimerState {
    start_session_with_category(SessionType::Custom, category, label, duration_secs, now)
}

fn start_session_with_category(
    session_type: SessionType,
    category: SessionCategory,
    label: String,
    duration_secs: u64,
    now: DateTime<Local>,
) -> TimerState {
    TimerState {
        status: TimerStatus::Running,
        session_type,
        label,
        duration_secs,
        started_at: Some(now),
        paused_remaining_secs: None,
        category,
        session_id: Some(crate::state::new_session_id()),
        history_recorded: false,
        notification_sent: false,
    }
}

pub fn pause(state: &TimerState, now: DateTime<Local>) -> TimerState {
    if state.status != TimerStatus::Running {
        return state.clone();
    }

    TimerState {
        status: TimerStatus::Paused,
        started_at: None,
        paused_remaining_secs: Some(remaining_secs_at(state, now)),
        ..state.clone()
    }
}

pub fn resume(state: &TimerState, now: DateTime<Local>) -> TimerState {
    if state.status != TimerStatus::Paused {
        return state.clone();
    }

    let remaining = state.paused_remaining_secs.unwrap_or(state.duration_secs);
    let elapsed_before_pause = state.duration_secs.saturating_sub(remaining);
    let started_at = now - chrono::Duration::seconds(elapsed_before_pause as i64);
    TimerState {
        status: TimerStatus::Running,
        started_at: Some(started_at),
        paused_remaining_secs: None,
        ..state.clone()
    }
}

pub fn stop() -> TimerState {
    TimerState::idle()
}

pub fn finish_if_due(state: &TimerState, now: DateTime<Local>) -> TimerState {
    if state.status == TimerStatus::Finished {
        return state.clone();
    }

    if state.status == TimerStatus::Running && remaining_secs_at(state, now) == 0 {
        return TimerState {
            status: TimerStatus::Finished,
            started_at: None,
            paused_remaining_secs: Some(0),
            ..state.clone()
        };
    }

    state.clone()
}

/// Instante nominal de conclusão. Para uma sessão atrasada, este é o instante
/// do vencimento, não o instante em que algum cliente consultou o daemon.
pub fn due_at(state: &TimerState) -> Option<DateTime<Local>> {
    if state.status != TimerStatus::Running {
        return None;
    }
    let started_at = state.started_at?;
    let seconds = i64::try_from(state.duration_secs).ok()?;
    started_at.checked_add_signed(chrono::Duration::seconds(seconds))
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
    use chrono::Duration;

    fn running(now: DateTime<Local>) -> TimerState {
        start_session(SessionType::Focus, "teste".to_string(), 60, now)
    }

    #[test]
    fn restante_de_estado_running_usa_timestamp() {
        let now = Local::now();
        let state = running(now - Duration::seconds(10));

        assert_eq!(remaining_secs_at(&state, now), 50);
    }

    #[test]
    fn restante_nao_fica_negativo() {
        let now = Local::now();
        let state = running(now - Duration::seconds(120));

        assert_eq!(remaining_secs_at(&state, now), 0);
    }

    #[test]
    fn pause_grava_restante() {
        let now = Local::now();
        let state = running(now - Duration::seconds(15));

        let paused = pause(&state, now);

        assert_eq!(paused.status, TimerStatus::Paused);
        assert_eq!(paused.paused_remaining_secs, Some(45));
        assert_eq!(paused.started_at, None);
    }

    #[test]
    fn resume_recria_started_at_preservando_restante() {
        let now = Local::now();
        let paused = TimerState {
            status: TimerStatus::Paused,
            session_type: SessionType::Focus,
            category: SessionCategory::Focus,
            label: "teste".to_string(),
            duration_secs: 60,
            started_at: None,
            paused_remaining_secs: Some(35),
            session_id: Some("session-test".to_string()),
            history_recorded: false,
            notification_sent: false,
        };

        let resumed = resume(&paused, now);

        assert_eq!(resumed.status, TimerStatus::Running);
        assert_eq!(resumed.duration_secs, 60);
        assert_eq!(remaining_secs_at(&resumed, now), 35);
        assert_eq!(resumed.paused_remaining_secs, None);
    }

    #[test]
    fn stop_retorna_idle() {
        assert_eq!(stop(), TimerState::idle());
    }

    #[test]
    fn finish_e_idempotente() {
        let now = Local::now();
        let state = running(now - Duration::seconds(60));

        let finished = finish_if_due(&state, now);
        let finished_again = finish_if_due(&finished, now + Duration::seconds(10));

        assert_eq!(finished.status, TimerStatus::Finished);
        assert_eq!(finished_again, finished);
    }

    #[test]
    fn finalizacao_preserva_instante_real_do_vencimento() {
        let started_at = Local::now();
        let state = running(started_at);
        let detected_at = started_at + Duration::seconds(75);

        assert_eq!(due_at(&state), Some(started_at + Duration::seconds(60)));
        assert_eq!(remaining_secs_at(&state, detected_at), 0);
    }

    #[test]
    fn customizado_valida_zero_limite_e_overflow() {
        assert!(duration_secs_from_minutes(0).is_err());
        assert_eq!(
            duration_secs_from_minutes(MAX_CUSTOM_MINUTES).unwrap(),
            MAX_CUSTOM_MINUTES * 60
        );
        assert!(duration_secs_from_minutes(MAX_CUSTOM_MINUTES + 1).is_err());
        assert!(duration_secs_from_minutes(u64::MAX).is_err());
    }

    #[test]
    fn start_por_args_rejeita_duracao_invalida_do_ipc() {
        let args = StartArgs {
            profile: None,
            break_session: false,
            custom: Some(u64::MAX),
            session_type: Some(CustomSessionType::Focus),
        };

        assert!(start_from_args(&args, Local::now()).is_err());
    }
}
