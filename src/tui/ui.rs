use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Style, Stylize},
    widgets::{Block, Gauge, Paragraph},
    Frame,
};

use crate::{state, tui::app::TuiApp};

pub fn render(frame: &mut Frame, app: &TuiApp) {
    let area = frame.area();
    let [header, body, gauge, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(3),
        Constraint::Length(4),
    ])
    .areas(area);

    frame.render_widget(
        Paragraph::new("Omarchy Pomo".bold())
            .centered()
            .block(Block::bordered()),
        header,
    );

    let body_text = if let Some(input) = &app.custom_input {
        let kind = input
            .session_type
            .map(|kind| match kind {
                crate::cli::CustomSessionType::Focus => "focus",
                crate::cli::CustomSessionType::Break => "break",
            })
            .unwrap_or("não escolhido");
        let error = app
            .error
            .as_ref()
            .map(|error| format!("\nErro: {error}"))
            .unwrap_or_default();
        format!(
            "Custom timer\nMinutos: {}\nTipo: {}\nDigite números, f=focus, b=break, Enter=iniciar, Esc=cancelar{}",
            input.minutes, kind, error
        )
    } else if let Some(error) = &app.error {
        format!("Erro: {error}")
    } else if let Some(state) = &app.state {
        format!(
            "Status: {}\nSessão: {}\nTipo: {}\nRestante: {}",
            status_name(&state.status),
            state.label,
            session_name(&state.session_type),
            format_duration(app.remaining_secs())
        )
    } else {
        "Conectando ao daemon...".to_string()
    };

    frame.render_widget(
        Paragraph::new(body_text).block(Block::bordered().title("Timer")),
        body,
    );

    frame.render_widget(
        Gauge::default()
            .block(Block::bordered().title("Progresso"))
            .gauge_style(Style::default().fg(Color::Green))
            .ratio(app.progress_ratio().clamp(0.0, 1.0)),
        gauge,
    );

    let history = app
        .summary
        .as_ref()
        .map(|summary| {
            format!(
                "Hoje: {} foco(s), {} focado, {} pausa(s)",
                summary.focus_sessions,
                format_duration(summary.focused_secs),
                summary.break_sessions
            )
        })
        .unwrap_or_else(|| "Hoje: sem resumo carregado".to_string());

    frame.render_widget(
        Paragraph::new(format!(
            "1 foco 25 | 2 foco 30 | 3 break | 4 custom | p pause/resume | s stop | q sair\n{history}"
        ))
        .block(Block::bordered().title("Atalhos")),
        footer,
    );
}

fn format_duration(secs: u64) -> String {
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

fn status_name(status: &state::TimerStatus) -> &'static str {
    match status {
        state::TimerStatus::Idle => "idle",
        state::TimerStatus::Running => "running",
        state::TimerStatus::Paused => "paused",
        state::TimerStatus::Finished => "finished",
    }
}

fn session_name(session_type: &state::SessionType) -> &'static str {
    match session_type {
        state::SessionType::Focus => "focus",
        state::SessionType::ShortBreak => "short_break",
        state::SessionType::Custom => "custom",
    }
}
