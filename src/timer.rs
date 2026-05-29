use anyhow::{bail, ensure};
use chrono::{DateTime, Local};

use crate::{
    cli::{CustomSessionType, StartArgs},
    state::{SessionType, TimerState, TimerStatus},
};

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
        ensure!(minutes > 0, "tempo customizado deve ser maior que zero");
        let session_type = match args.session_type {
            Some(CustomSessionType::Focus) => "Focus",
            Some(CustomSessionType::Break) => "Break",
            None => bail!("--custom exige --type <focus|break>"),
        };
        return Ok(start_session(
            SessionType::Custom,
            format!("Custom {session_type} ({minutes} min)"),
            minutes * 60,
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
    TimerState {
        status: TimerStatus::Running,
        session_type,
        label,
        duration_secs,
        started_at: Some(now),
        paused_remaining_secs: None,
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
    TimerState {
        status: TimerStatus::Running,
        duration_secs: remaining,
        started_at: Some(now),
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
            label: "teste".to_string(),
            duration_secs: 60,
            started_at: None,
            paused_remaining_secs: Some(35),
        };

        let resumed = resume(&paused, now);

        assert_eq!(resumed.status, TimerStatus::Running);
        assert_eq!(resumed.duration_secs, 35);
        assert_eq!(resumed.started_at, Some(now));
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
}
