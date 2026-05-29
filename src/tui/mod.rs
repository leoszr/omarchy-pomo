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
        refresh(&mut app, paths);
        terminal.draw(|frame| ui::render(frame, &app))?;

        if app.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    match events::action_for_key(key, app.state.as_ref()) {
                        TuiAction::Request(request) => match ipc::request(paths, &request) {
                            Ok(response) => app.apply_response(response),
                            Err(error) => app.set_error(format!("{error:#}")),
                        },
                        TuiAction::Quit => app.should_quit = true,
                        TuiAction::None => {}
                    }
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }

    Ok(())
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
