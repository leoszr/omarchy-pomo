use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};

use crate::{
    cli::CustomSessionType,
    formatting,
    state::{SessionCategory, TimerState, TimerStatus},
    tui::app::TuiApp,
};

/// Keep the terminal in charge of the surface and of the default foreground.
/// Only ANSI slots are used for semantic accents below.
fn terminal_style() -> Style {
    Style::default().fg(Color::Reset).bg(Color::Reset)
}

fn muted_style() -> Style {
    terminal_style().fg(Color::DarkGray)
}

fn state_style(state: Option<&TimerState>) -> Style {
    let color = match state {
        Some(state) if state.status == TimerStatus::Paused => Color::Yellow,
        Some(state) if state.status == TimerStatus::Finished => Color::Green,
        Some(state) if state.category == SessionCategory::Break => Color::Magenta,
        Some(_) => Color::Blue,
        None => Color::DarkGray,
    };
    terminal_style().fg(color)
}

fn error_style() -> Style {
    terminal_style().fg(Color::Red)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutKind {
    Tiny,
    Compact,
    Standard,
    Wide,
}

impl LayoutKind {
    fn for_area(area: Rect) -> Self {
        if area.width < 32 || area.height < 10 {
            Self::Tiny
        } else if area.width < 52 || area.height < 16 {
            Self::Compact
        } else if area.width < 85 || area.height < 24 {
            Self::Standard
        } else {
            Self::Wide
        }
    }

    fn show_summary(self, area: Rect) -> bool {
        !matches!(self, Self::Tiny | Self::Compact) && area.height >= 10
    }

    fn uses_block_timer(self, area: Rect) -> bool {
        !matches!(self, Self::Tiny | Self::Compact) && area.width >= 52 && area.height >= 18
    }
}

pub fn render(frame: &mut Frame, app: &TuiApp) {
    let area = frame.area();
    let kind = LayoutKind::for_area(area);
    let frame_style = terminal_style();
    let outer = Block::bordered()
        .title(Span::styled(" Omarchy Pomo ", terminal_style().bold()))
        .border_style(muted_style())
        .style(frame_style);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    if kind == LayoutKind::Tiny {
        render_tiny(frame, inner, app);
    } else if app.custom_input.is_some() {
        render_custom(frame, inner, app, kind);
    } else {
        render_timer(frame, inner, app, kind);
    }
}

fn render_tiny(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let state = app.state.as_ref();
    let timer = app
        .state
        .as_ref()
        .map(|_| formatting::duration(app.remaining_secs()))
        .unwrap_or_else(|| "--:--".to_string());
    let status = state.map(status_label).unwrap_or("PRONTO");
    let action = match state.map(|state| &state.status) {
        Some(TimerStatus::Running) => "[p] Pausar  [q] Sair",
        Some(TimerStatus::Paused) => "[p] Retomar [q] Sair",
        _ => "[1] 25m  [4] Custom",
    };
    let mut lines = vec![
        Line::from(vec![
            Span::styled(timer, state_style(state)),
            Span::styled(" · ", muted_style()),
            Span::styled(status, state_style(state).bold()),
        ]),
        Line::from(Span::styled(action, terminal_style().bold())),
    ];
    if let Some(error) = &app.error {
        lines.push(Line::from(Span::styled(short_error(error), error_style())));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(ratatui::layout::Alignment::Center)
            .style(terminal_style()),
        area,
    );
}

fn render_timer(frame: &mut Frame, area: Rect, app: &TuiApp, kind: LayoutKind) {
    let action_lines = actions(app.state.as_ref(), area.width);
    let show_summary = kind.show_summary(area);
    let show_error = app.error.is_some() && area.height >= 8;
    let action_height = action_lines.len() as u16;
    let mut constraints = vec![
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(action_height),
    ];
    if show_summary {
        constraints.push(Constraint::Length(1));
    }
    if show_error {
        constraints.push(Constraint::Length(1));
    }
    let areas = Layout::vertical(constraints).split(area);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("foco sem ruído", muted_style()),
            Span::styled("  ·  ", muted_style()),
            Span::styled(
                connection_label(app),
                if app.state.is_some() {
                    terminal_style()
                } else {
                    error_style()
                },
            ),
        ]))
        .style(terminal_style()),
        areas[0],
    );

    render_stage(frame, areas[1], app, kind.uses_block_timer(area));
    render_progress(frame, areas[2], app, kind == LayoutKind::Wide);
    frame.render_widget(
        Paragraph::new(action_lines).style(terminal_style()),
        areas[3],
    );

    let mut next = 4;
    if show_summary {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(summary_line(app), muted_style())))
                .style(terminal_style()),
            areas[next],
        );
        next += 1;
    }
    if show_error {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(
                    "Erro: {}",
                    short_error(app.error.as_deref().unwrap_or_default())
                ),
                error_style(),
            )))
            .style(terminal_style()),
            areas[next],
        );
    }
}

fn render_stage(frame: &mut Frame, area: Rect, app: &TuiApp, block_timer: bool) {
    let state = app.state.as_ref();
    let mut lines = Vec::new();
    if let Some(state) = state {
        let semantic = state_style(Some(state));
        lines.push(Line::from(Span::styled(
            status_label(state),
            semantic.bold(),
        )));
        if block_timer {
            lines.extend(block_timer_lines(app.remaining_secs(), semantic));
        } else {
            lines.push(Line::from(Span::styled(
                formatting::duration(app.remaining_secs()),
                semantic.bold(),
            )));
        }
        lines.push(Line::from(Span::styled(
            truncate_label(&state.label, area.width.saturating_sub(2) as usize),
            terminal_style().bold(),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "Daemon indisponível",
            error_style().bold(),
        )));
        lines.push(Line::from(Span::styled(
            "inicie com: omarchy-pomo daemon",
            muted_style(),
        )));
    }
    frame.render_widget(
        Paragraph::new(center_lines(lines, area.height))
            .alignment(ratatui::layout::Alignment::Center)
            .style(terminal_style()),
        area,
    );
}

fn render_progress(frame: &mut Frame, area: Rect, app: &TuiApp, show_percent: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let percent = (app.progress_ratio().clamp(0.0, 1.0) * 100.0).round() as u16;
    let percent_width = if show_percent { 5 } else { 0 };
    let track_width = area.width.saturating_sub(percent_width) as usize;
    let completed = ((track_width as f64) * app.progress_ratio().clamp(0.0, 1.0)).round() as usize;
    let remaining = track_width.saturating_sub(completed);
    let color = state_style(app.state.as_ref());
    let mut spans = vec![Span::styled("━".repeat(completed), color)];
    spans.push(Span::styled("─".repeat(remaining), muted_style()));
    if show_percent {
        spans.push(Span::styled(format!(" {percent:>3}%"), muted_style()));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(terminal_style()),
        area,
    );
}

fn render_custom(frame: &mut Frame, area: Rect, app: &TuiApp, _kind: LayoutKind) {
    let Some(input) = app.custom_input.as_ref() else {
        return;
    };
    let mut lines = vec![Line::from(Span::styled(
        "Nova sessão",
        terminal_style().bold(),
    ))];
    let mut duration_spans = vec![Span::styled("Duração  ", muted_style())];
    duration_spans.push(Span::styled("[", terminal_style()));
    duration_spans.push(Span::styled(
        input.minutes.clone(),
        terminal_style().fg(Color::Blue),
    ));
    duration_spans.push(Span::styled("▌", terminal_style().fg(Color::Blue).bold()));
    duration_spans.push(Span::styled(
        "_".repeat(4usize.saturating_sub(input.minutes.len())),
        terminal_style().fg(Color::Blue),
    ));
    duration_spans.push(Span::styled("] min", terminal_style()));
    lines.push(Line::from(duration_spans));

    let focus_style = if input.session_type == Some(CustomSessionType::Focus) {
        terminal_style().add_modifier(Modifier::REVERSED)
    } else {
        terminal_style()
    };
    let break_style = if input.session_type == Some(CustomSessionType::Break) {
        terminal_style().add_modifier(Modifier::REVERSED)
    } else {
        terminal_style()
    };
    lines.push(Line::from(vec![
        Span::styled("Tipo     ", muted_style()),
        Span::styled("[Foco]", focus_style),
        Span::styled("  ", terminal_style()),
        Span::styled("Pausa", break_style),
    ]));
    if let Some(error) = &app.error {
        lines.push(Line::from(Span::styled(
            format!("Erro: {}", short_error(error)),
            error_style(),
        )));
    }
    lines.push(Line::from(Span::styled(
        "Enter iniciar · Esc voltar",
        muted_style(),
    )));

    frame.render_widget(
        Paragraph::new(center_lines(lines, area.height))
            .alignment(ratatui::layout::Alignment::Center)
            .style(terminal_style()),
        area,
    );
}

fn block_timer_lines(seconds: u64, style: Style) -> Vec<Line<'static>> {
    const DIGITS: [[&str; 5]; 10] = [
        ["███", "█ █", "█ █", "█ █", "███"],
        [" ██", "██ ", " ██", " ██", "███"],
        ["███", "  █", "███", "█  ", "███"],
        ["███", "  █", " ██", "  █", "███"],
        ["█ █", "█ █", "███", "  █", "  █"],
        ["███", "█  ", "███", "  █", "███"],
        [" ██", "█  ", "███", "█ █", "███"],
        ["███", "  █", "  █", "  █", "  █"],
        ["███", "█ █", "███", "█ █", "███"],
        ["███", "█ █", "███", "  █", "██ "],
    ];
    let display = formatting::duration(seconds);
    (0..5)
        .map(|row| {
            let mut spans = Vec::new();
            for (index, character) in display.chars().enumerate() {
                let glyph = match character {
                    '0'..='9' => DIGITS[character as usize - '0' as usize][row],
                    ':' if row == 1 || row == 3 => "·",
                    ':' => " ",
                    _ => " ",
                };
                spans.push(Span::styled(glyph, style));
                if index < 4 {
                    spans.push(Span::raw(" "));
                }
            }
            Line::from(spans)
        })
        .collect()
}

fn center_lines(mut lines: Vec<Line<'static>>, height: u16) -> Vec<Line<'static>> {
    let missing = height.saturating_sub(lines.len() as u16) as usize;
    let top = missing / 2;
    let bottom = missing.saturating_sub(top);
    let mut centered = Vec::with_capacity(lines.len() + missing);
    centered.extend((0..top).map(|_| Line::from("")));
    centered.append(&mut lines);
    centered.extend((0..bottom).map(|_| Line::from("")));
    centered
}

fn actions(state: Option<&TimerState>, width: u16) -> Vec<Line<'static>> {
    let compact = width < 64;
    match state.map(|state| &state.status) {
        Some(TimerStatus::Running) => {
            if compact {
                vec![
                    action_line(&[("[p]", " Pausar"), ("  ", "[s] Parar")]),
                    action_line(&[("[q]", " Sair")]),
                ]
            } else {
                vec![action_line(&[
                    ("[p]", " Pausar  "),
                    ("[s]", " Parar  "),
                    ("[q]", " Sair"),
                ])]
            }
        }
        Some(TimerStatus::Paused) => {
            if compact {
                vec![
                    action_line(&[("[p]", " Retomar"), ("  ", "[s] Parar")]),
                    action_line(&[("[q]", " Sair")]),
                ]
            } else {
                vec![action_line(&[
                    ("[p]", " Retomar  "),
                    ("[s]", " Parar  "),
                    ("[q]", " Sair"),
                ])]
            }
        }
        _ if compact => vec![
            action_line(&[("[1]", " 25 min  "), ("[2]", " 30 min")]),
            action_line(&[("[3]", " Pausa  "), ("[4]", " Custom")]),
        ],
        _ => vec![action_line(&[
            ("[1]", " 25 min  "),
            ("[2]", " 30 min  "),
            ("[3]", " Pausa  "),
            ("[4]", " Personalizar"),
        ])],
    }
}

fn action_line(parts: &[(&'static str, &'static str)]) -> Line<'static> {
    let mut spans = Vec::with_capacity(parts.len() * 2);
    for (key, label) in parts {
        spans.push(Span::styled(*key, terminal_style().bold()));
        spans.push(Span::styled(*label, terminal_style()));
    }
    Line::from(spans)
}

fn status_label(state: &TimerState) -> &'static str {
    match state.status {
        TimerStatus::Running if state.category == SessionCategory::Break => "DESCANSO",
        TimerStatus::Running => "FOCO",
        TimerStatus::Paused => "PAUSA",
        TimerStatus::Finished => "CONCLUÍDO",
        TimerStatus::Idle => "PRONTO",
    }
}

fn connection_label(app: &TuiApp) -> &'static str {
    if app.state.is_some() {
        "conectado"
    } else {
        "daemon offline"
    }
}

fn summary_line(app: &TuiApp) -> String {
    let Some(summary) = &app.summary else {
        return "Histórico indisponível".to_string();
    };
    format!(
        "Hoje  {} focos · {} · {} pausas",
        summary.focus_sessions,
        summary_duration(summary.focused_secs),
        summary.break_sessions
    )
}

fn summary_duration(seconds: u64) -> String {
    let hours = seconds / 3_600;
    let minutes = (seconds % 3_600) / 60;
    if hours > 0 {
        format!("{hours}h{minutes:02}")
    } else {
        format!("{minutes}m")
    }
}

fn truncate_label(label: &str, max_chars: usize) -> String {
    if label.chars().count() <= max_chars {
        return label.to_string();
    }
    label
        .chars()
        .take(max_chars.saturating_sub(1))
        .chain(std::iter::once('…'))
        .collect()
}

fn short_error(error: &str) -> String {
    // `TuiApp` adds the source when more than one subsystem failed. Keep that
    // context visible instead of hiding it behind a generic connection error.
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;
    use ratatui::{backend::TestBackend, Terminal};

    fn state(status: TimerStatus, category: SessionCategory) -> TimerState {
        TimerState {
            status,
            session_type: if category == SessionCategory::Break {
                crate::state::SessionType::ShortBreak
            } else {
                crate::state::SessionType::Focus
            },
            label: "Sessão de teste muito longa para validar truncamento".to_string(),
            duration_secs: 1_500,
            started_at: Some(Local::now()),
            paused_remaining_secs: None,
            category,
            session_id: Some("test".to_string()),
            history_recorded: false,
            notification_sent: false,
        }
    }

    fn draw(app: &TuiApp, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, app)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn renderiza_todos_os_tamanhos_de_referencia_sem_panic() {
        let mut app = TuiApp::default();
        app.state = Some(state(TimerStatus::Running, SessionCategory::Focus));
        for (width, height) in [(32, 10), (52, 16), (70, 24), (100, 30)] {
            let output = draw(&app, width, height);
            assert_eq!(output.lines().count(), height as usize);
            assert!(output.contains("FOCO"), "saída {width}x{height}: {output}");
        }
    }

    #[test]
    fn renderiza_todos_os_estados_sem_panic() {
        for (status, label) in [
            (TimerStatus::Idle, "PRONTO"),
            (TimerStatus::Paused, "PAUSA"),
            (TimerStatus::Finished, "CONCLUÍDO"),
        ] {
            let mut app = TuiApp::default();
            app.state = Some(state(status, SessionCategory::Focus));
            for (width, height) in [(32, 10), (52, 16), (70, 24), (100, 30)] {
                let output = draw(&app, width, height);
                assert!(output.contains(label), "saída {width}x{height}: {output}");
            }
        }
    }

    #[test]
    fn layout_tiny_mantem_fallback_de_uma_linha_e_comandos() {
        let mut app = TuiApp::default();
        app.state = Some(state(TimerStatus::Paused, SessionCategory::Focus));
        let output = draw(&app, 31, 9);
        assert!(output.contains("PAUSA"));
        assert!(output.contains("[p]"));
        assert!(!output.contains("███"));
    }

    #[test]
    fn layouts_semanticos_usam_ansi_e_reset_sem_fundo_rgb() {
        let mut app = TuiApp::default();
        app.state = Some(state(TimerStatus::Running, SessionCategory::Break));
        let backend = TestBackend::new(70, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let timer_cell = buffer
            .content
            .iter()
            .find(|cell| cell.symbol() == "█")
            .expect("dígito block");
        assert_eq!(timer_cell.fg, Color::Magenta);
        assert_eq!(timer_cell.bg, Color::Reset);
        assert_eq!(buffer[(0, 0)].bg, Color::Reset);
    }

    #[test]
    fn estado_valido_permanece_visivel_com_erro() {
        let mut app = TuiApp::default();
        app.state = Some(state(TimerStatus::Running, SessionCategory::Focus));
        app.error = Some("daemon caiu".to_string());
        let output = draw(&app, 70, 24);
        assert!(output.contains("FOCO"));
        assert!(output.contains("Erro: daemon caiu"));
    }

    #[test]
    fn formulario_inline_preserva_valores_e_mostra_erro() {
        let mut app = TuiApp::default();
        app.custom_input = Some(crate::tui::app::CustomInput {
            minutes: "45".to_string(),
            session_type: Some(CustomSessionType::Focus),
        });
        app.error = Some("digite minutos válidos".to_string());
        let output = draw(&app, 52, 16);
        assert!(output.contains("Nova sessão"));
        assert!(output.contains("45"));
        assert!(output.contains("Foco"));
        assert!(output.contains("Erro: digite minutos válidos"));
    }

    #[test]
    fn progresso_respeita_categoria_da_sessao() {
        let mut app = TuiApp::default();
        app.state = Some(state(TimerStatus::Running, SessionCategory::Break));
        app.summary = Some(crate::history::DailySummary::default());
        let output = draw(&app, 100, 30);
        assert!(output.contains("DESCANSO"));
        assert!(output.contains("Hoje"));
        assert!(!output.contains("Progresso"));
    }
}
