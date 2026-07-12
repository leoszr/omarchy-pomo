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
    tui::app::{CustomInput, TuiApp},
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
        Some(state) if state.status == TimerStatus::Idle => Color::DarkGray,
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

    fn show_summary(self) -> bool {
        !matches!(self, Self::Tiny | Self::Compact)
    }

    /// Thresholds use the external terminal rectangle. The inner rectangle
    /// is two cells smaller because of the single outer frame.
    fn uses_block_timer(self, outer: Rect) -> bool {
        !matches!(self, Self::Tiny | Self::Compact) && outer.width >= 52 && outer.height >= 18
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

    if let Some(input) = app.custom_input.as_ref() {
        if kind == LayoutKind::Tiny {
            render_custom_tiny(frame, inner, app, input);
        } else {
            render_custom(frame, inner, app, kind);
        }
    } else if kind == LayoutKind::Tiny {
        render_tiny(frame, inner, app);
    } else {
        render_timer(frame, inner, app, kind, area);
    }
}

fn render_tiny(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let Some(state) = app.state.as_ref() else {
        let mut lines = vec![
            Line::from(Span::styled("Daemon offline", error_style().bold())),
            Line::from(Span::styled("omarchy-pomo daemon", muted_style())),
            action_line(&[("[q]", " Sair")]),
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
        return;
    };

    let timer = formatting::duration(app.remaining_secs());
    let status = status_label(state);
    let action_lines = match state.status {
        TimerStatus::Running => vec![
            action_line(&[("[p]", " Pausar  "), ("[s]", " Parar")]),
            action_line(&[("[q]", " Sair")]),
        ],
        TimerStatus::Paused => vec![
            action_line(&[("[p]", " Retomar  "), ("[s]", " Parar")]),
            action_line(&[("[q]", " Sair")]),
        ],
        TimerStatus::Idle | TimerStatus::Finished => vec![
            action_line(&[("[1]", " 25m  "), ("[2]", " 30m")]),
            action_line(&[("[3]", " Pausa  "), ("[4]", " Custom")]),
            action_line(&[("[q]", " Sair")]),
        ],
    };
    let mut lines = vec![Line::from(vec![
        Span::styled(timer, state_style(Some(state))),
        Span::styled(" · ", muted_style()),
        Span::styled(status, state_style(Some(state)).bold()),
    ])];
    lines.extend(action_lines);
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

fn render_timer(frame: &mut Frame, area: Rect, app: &TuiApp, kind: LayoutKind, outer: Rect) {
    let action_lines = actions(app.state.as_ref(), area.width);
    let show_summary = app.state.is_some() && kind.show_summary();
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
            Span::styled(connection_label(app), connection_style(app)),
        ]))
        .style(terminal_style()),
        areas[0],
    );

    render_stage(frame, areas[1], app, kind.uses_block_timer(outer));
    if app.state.is_some() {
        render_progress(frame, areas[2], app, kind == LayoutKind::Wide);
    }
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

fn render_custom_tiny(frame: &mut Frame, area: Rect, app: &TuiApp, input: &CustomInput) {
    let mut lines = vec![Line::from(Span::styled(
        "Nova sessão",
        terminal_style().bold(),
    ))];
    lines.push(custom_duration_line(input));
    lines.push(custom_type_line(input, true));
    if let Some(error) = &app.error {
        lines.push(Line::from(Span::styled(short_error(error), error_style())));
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

fn render_custom(frame: &mut Frame, area: Rect, app: &TuiApp, _kind: LayoutKind) {
    let Some(input) = app.custom_input.as_ref() else {
        return;
    };
    let mut lines = vec![Line::from(Span::styled(
        "Nova sessão",
        terminal_style().bold(),
    ))];
    lines.push(custom_duration_line(input));
    lines.push(custom_type_line(input, false));
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

fn custom_duration_line(input: &CustomInput) -> Line<'static> {
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
    Line::from(duration_spans)
}

fn custom_type_line(input: &CustomInput, tiny: bool) -> Line<'static> {
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
    if tiny {
        Line::from(vec![
            Span::styled("f ", terminal_style().bold()),
            Span::styled("Foco", focus_style),
            Span::styled("  b ", terminal_style().bold()),
            Span::styled("Pausa", break_style),
        ])
    } else {
        Line::from(vec![
            Span::styled("Tipo     ", muted_style()),
            Span::styled("[Foco]", focus_style),
            Span::styled("  ", terminal_style()),
            Span::styled("Pausa", break_style),
        ])
    }
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
    let display_len = display.chars().count();
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
                if index + 1 < display_len {
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
        None => vec![action_line(&[("[q]", " Sair")])],
        Some(TimerStatus::Running) => {
            if compact {
                vec![
                    action_line(&[("[p]", " Pausar  "), ("[s]", " Parar")]),
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
                    action_line(&[("[p]", " Retomar  "), ("[s]", " Parar")]),
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
        Some(TimerStatus::Idle | TimerStatus::Finished) if compact => vec![
            action_line(&[("[1]", " 25 min  "), ("[2]", " 30 min")]),
            action_line(&[("[3]", " Pausa  "), ("[4]", " Custom")]),
            action_line(&[("[q]", " Sair")]),
        ],
        Some(TimerStatus::Idle | TimerStatus::Finished) => vec![
            action_line(&[
                ("[1]", " 25 min  "),
                ("[2]", " 30 min  "),
                ("[3]", " Pausa  "),
                ("[4]", " Personalizar"),
            ]),
            action_line(&[("[q]", " Sair")]),
        ],
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
    if app.state.is_some() && app.status_error_active() {
        "estado desatualizado"
    } else if app.state.is_some() {
        "conectado"
    } else {
        "daemon offline"
    }
}

fn connection_style(app: &TuiApp) -> Style {
    if app.state.is_none() {
        error_style()
    } else if app.status_error_active() {
        terminal_style().fg(Color::Yellow)
    } else {
        terminal_style()
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
        for (width, height) in [(32, 10), (52, 16), (52, 18), (70, 24), (100, 30)] {
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
            for (width, height) in [(32, 10), (52, 16), (52, 18), (70, 24), (100, 30)] {
                let output = draw(&app, width, height);
                assert!(output.contains(label), "saída {width}x{height}: {output}");
            }
        }
    }

    #[test]
    fn thresholds_usam_area_externa_nos_limites_documentados() {
        assert_eq!(
            LayoutKind::for_area(Rect::new(0, 0, 31, 9)),
            LayoutKind::Tiny
        );
        assert_eq!(
            LayoutKind::for_area(Rect::new(0, 0, 32, 10)),
            LayoutKind::Compact
        );
        assert_eq!(
            LayoutKind::for_area(Rect::new(0, 0, 52, 16)),
            LayoutKind::Standard
        );
        assert!(!LayoutKind::Standard.uses_block_timer(Rect::new(0, 0, 52, 16)));
        assert!(LayoutKind::Standard.uses_block_timer(Rect::new(0, 0, 52, 18)));
        assert!(LayoutKind::Standard.uses_block_timer(Rect::new(0, 0, 70, 24)));
        assert_eq!(
            LayoutKind::for_area(Rect::new(0, 0, 100, 30)),
            LayoutKind::Wide
        );
        assert!(LayoutKind::Wide.uses_block_timer(Rect::new(0, 0, 100, 30)));
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
    fn formulario_custom_funciona_no_tiny_e_no_compacto() {
        let mut app = TuiApp::default();
        app.custom_input = Some(crate::tui::app::CustomInput {
            minutes: "45".to_string(),
            session_type: Some(CustomSessionType::Focus),
        });
        for (width, height) in [(31, 9), (52, 16)] {
            let output = draw(&app, width, height);
            assert!(
                output.contains("Nova sessão"),
                "saída {width}x{height}: {output}"
            );
            assert!(output.contains("45"), "saída {width}x{height}: {output}");
            assert!(output.contains("Foco"), "saída {width}x{height}: {output}");
            assert!(output.contains("Enter"), "saída {width}x{height}: {output}");
        }
    }

    #[test]
    fn daemon_offline_nao_oferece_presets_no_tiny_nem_no_padrao() {
        let app = TuiApp::default();
        for (width, height) in [(31, 9), (52, 16)] {
            let output = draw(&app, width, height);
            assert!(output.contains("Daemon offline") || output.contains("Daemon indisponível"));
            assert!(output.contains("omarchy-pomo daemon"));
            assert!(
                output.contains("[q] Sair"),
                "saída {width}x{height}: {output}"
            );
            assert!(!output.contains("[1]"), "saída {width}x{height}: {output}");
            assert!(!output.contains("[4]"), "saída {width}x{height}: {output}");
        }
    }

    #[test]
    fn idle_e_neutro_e_atalhos_tiny_mostram_acoes_essenciais() {
        let mut app = TuiApp::default();
        app.state = Some(state(TimerStatus::Idle, SessionCategory::Focus));
        let backend = TestBackend::new(32, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let pronto = buffer
            .content
            .iter()
            .find(|cell| cell.symbol() == "P" && cell.fg == Color::DarkGray)
            .expect("rótulo PRONTO");
        assert_eq!(pronto.fg, Color::DarkGray);
        let output = draw(&app, 32, 10);
        assert!(output.contains("[1]"));
        assert!(output.contains("[4]"));
        assert!(output.contains("[q]"), "saída tiny idle: {output}");

        let output = draw(&app, 70, 24);
        assert!(output.contains("[q] Sair"), "saída wide idle: {output}");

        app.state = Some(state(TimerStatus::Finished, SessionCategory::Focus));
        let output = draw(&app, 70, 24);
        assert!(output.contains("[q] Sair"), "saída wide finished: {output}");
    }

    #[test]
    fn timer_block_separa_todos_os_digitos_de_duracoes_longas() {
        for (seconds, expected_width, expected_spans) in [(7_200, 21, 11), (86_400, 25, 13)] {
            let lines = block_timer_lines(seconds, terminal_style());
            assert_eq!(lines.len(), 5);
            assert!(lines.iter().all(|line| line.width() == expected_width));
            assert!(lines.iter().all(|line| line.spans.len() == expected_spans));
        }
    }

    #[test]
    fn falha_de_status_marca_estado_stale_e_recupera_conexao() {
        let mut app = TuiApp::default();
        app.state = Some(state(TimerStatus::Running, SessionCategory::Focus));
        app.apply_response_from(
            crate::tui::app::ErrorSource::Status,
            crate::ipc::IpcResponse::Error {
                message: "daemon caiu".to_string(),
            },
        );
        let stale = draw(&app, 52, 16);
        assert!(stale.contains("estado desatualizado"));
        assert!(stale.contains("FOCO"));

        app.apply_response_from(
            crate::tui::app::ErrorSource::Status,
            crate::ipc::IpcResponse::State {
                state: app.state.clone().expect("estado anterior"),
            },
        );
        let recovered = draw(&app, 52, 16);
        assert!(recovered.contains("conectado"));
        assert!(!recovered.contains("estado desatualizado"));
    }

    #[test]
    fn teclas_de_acao_compactas_mantem_s_como_tecla_bold() {
        let mut app = TuiApp::default();
        app.state = Some(state(TimerStatus::Running, SessionCategory::Focus));
        let backend = TestBackend::new(52, 16);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let s_key = buffer
            .content
            .iter()
            .find(|cell| cell.symbol() == "s" && cell.modifier.contains(Modifier::BOLD))
            .expect("tecla s");
        assert!(s_key.modifier.contains(Modifier::BOLD));
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
