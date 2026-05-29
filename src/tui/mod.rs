pub mod app;
pub mod events;
pub mod ui;

use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};

use crate::{ipc, state::StatePaths};
use app::TuiApp;
use events::TuiAction;

pub fn run() -> anyhow::Result<()> {
    let paths = StatePaths::new()?;
    let mut terminal = ratatui::init();
    let result = run_app(&mut terminal, &paths);
    ratatui::restore();
    result
}

fn run_app(terminal: &mut ratatui::DefaultTerminal, paths: &StatePaths) -> anyhow::Result<()> {
    let mut app = TuiApp::default();

    loop {
        if app.custom_input.is_none() {
            refresh(&mut app, paths);
        }
        terminal.draw(|frame| ui::render(frame, &app))?;

        if app.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(250))? {
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
        Err(error) => app.set_error(format!("{error:#}")),
    }
}

fn refresh(app: &mut TuiApp, paths: &StatePaths) {
    match ipc::request(paths, &ipc::IpcRequest::Status) {
        Ok(response) => app.apply_response(response),
        Err(error) => app.set_error(format!("{error:#}")),
    }
    match ipc::request(paths, &ipc::IpcRequest::History) {
        Ok(response) => app.apply_response(response),
        Err(error) => app.set_error(format!("{error:#}")),
    }
}
