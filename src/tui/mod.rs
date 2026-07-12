pub mod app;
pub mod events;
pub mod ui;

use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyEventKind};

use crate::{ipc, state::StatePaths};
use app::{ErrorSource, TuiApp};
use events::TuiAction;

const VISUAL_FRAME_INTERVAL: Duration = Duration::from_millis(100);
const STATUS_REFRESH_INTERVAL: Duration = Duration::from_secs(1);
const HISTORY_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug)]
struct RefreshSchedule {
    next_status: Instant,
    next_history: Instant,
}

impl RefreshSchedule {
    fn new(now: Instant) -> Self {
        Self {
            next_status: now,
            next_history: now,
        }
    }

    fn status_due(&self, now: Instant) -> bool {
        now >= self.next_status
    }

    fn history_due(&self, now: Instant) -> bool {
        now >= self.next_history
    }

    fn mark_status_refreshed(&mut self, now: Instant) {
        self.next_status = now + STATUS_REFRESH_INTERVAL;
    }

    fn mark_history_refreshed(&mut self, now: Instant) {
        self.next_history = now + HISTORY_REFRESH_INTERVAL;
    }
}

pub fn run() -> anyhow::Result<()> {
    let paths = StatePaths::new()?;
    let mut terminal = ratatui::init();
    let result = run_app(&mut terminal, &paths);
    ratatui::restore();
    result
}

fn run_app(terminal: &mut ratatui::DefaultTerminal, paths: &StatePaths) -> anyhow::Result<()> {
    let mut app = TuiApp::default();
    let mut schedule = RefreshSchedule::new(Instant::now());

    loop {
        let now = Instant::now();
        if app.custom_input.is_none() {
            let status_due = schedule.status_due(now);
            let history_due = schedule.history_due(now);
            let mut history_refreshed = false;
            if status_due {
                let completed = refresh_status(&mut app, paths);
                schedule.mark_status_refreshed(Instant::now());
                if completed {
                    refresh_history(&mut app, paths);
                    schedule.mark_history_refreshed(Instant::now());
                    history_refreshed = true;
                }
            }
            if history_due && !history_refreshed {
                refresh_history(&mut app, paths);
                schedule.mark_history_refreshed(Instant::now());
            }
        }
        terminal.draw(|frame| ui::render(frame, &app))?;

        if app.should_quit {
            break;
        }

        if event::poll(VISUAL_FRAME_INTERVAL)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    let action = if app.custom_input.is_some() {
                        events::action_for_custom_key(key)
                    } else {
                        events::action_for_key(key, app.state.as_ref())
                    };
                    handle_action(&mut app, paths, action);
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }

    Ok(())
}

fn handle_action(app: &mut TuiApp, paths: &StatePaths, action: TuiAction) {
    match action {
        TuiAction::Request(request) => send_request(app, paths, &request),
        TuiAction::BeginCustom => app.begin_custom_input(),
        TuiAction::PushCustomDigit(digit) => app.push_custom_digit(digit),
        TuiAction::PopCustomDigit => app.pop_custom_digit(),
        TuiAction::SetCustomType(session_type) => app.set_custom_type(session_type),
        TuiAction::SubmitCustom => match app.custom_start_args() {
            Ok(args) => {
                app.cancel_custom_input();
                send_request(app, paths, &ipc::IpcRequest::Start { args });
            }
            Err(error) => app.set_error(error),
        },
        TuiAction::CancelCustom => app.cancel_custom_input(),
        TuiAction::Quit => app.should_quit = true,
        TuiAction::None => {}
    }
}

fn send_request(app: &mut TuiApp, paths: &StatePaths, request: &ipc::IpcRequest) {
    match ipc::request(paths, request) {
        Ok(response) => app.apply_response(response),
        Err(error) => app.set_error_source(ErrorSource::Action, format!("{error:#}")),
    }
}

fn refresh_status(app: &mut TuiApp, paths: &StatePaths) -> bool {
    let was_finished = app
        .state
        .as_ref()
        .is_some_and(|state| state.status == crate::state::TimerStatus::Finished);
    let mut completed = false;
    match ipc::request(paths, &ipc::IpcRequest::Status) {
        Ok(response) => {
            app.apply_response_from(ErrorSource::Status, response);
            completed = !was_finished
                && app
                    .state
                    .as_ref()
                    .is_some_and(|state| state.status == crate::state::TimerStatus::Finished);
        }
        Err(error) => app.set_error_source(ErrorSource::Status, format!("{error:#}")),
    }
    completed
}

fn refresh_history(app: &mut TuiApp, paths: &StatePaths) {
    match ipc::request(paths, &ipc::IpcRequest::History) {
        Ok(response) => app.apply_response_from(ErrorSource::History, response),
        Err(error) => app.set_error_source(ErrorSource::History, format!("{error:#}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refreshes_separados_nao_pollam_quatro_vezes_por_segundo() {
        let start = Instant::now();
        let mut schedule = RefreshSchedule::new(start);
        assert!(schedule.status_due(start));
        assert!(schedule.history_due(start));

        schedule.mark_status_refreshed(start);
        schedule.mark_history_refreshed(start);

        assert!(!schedule.status_due(start + Duration::from_millis(999)));
        assert!(schedule.status_due(start + Duration::from_secs(1)));
        assert!(!schedule.history_due(start + Duration::from_secs(4)));
        assert!(schedule.history_due(start + Duration::from_secs(5)));
    }
}
