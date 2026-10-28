use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, TableState};

use crate::app::{App, Mode};

/// Herdr's Braille spinner for agents that are actively working.
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [input_area, heading_area, list_area, footer_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    let count = format!(" {}/{}", app.filtered.len(), app.agents.len());
    let [search_area, count_area] = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(u16::try_from(count.len()).unwrap_or(u16::MAX)),
    ])
    .areas(input_area);
    draw_input(frame, app, search_area);
    frame.render_widget(Paragraph::new(count).right_aligned().dim(), count_area);
    frame.render_widget(
        Table::new(
            [Row::new(["#", "", "Thread", "Workspace", "Agent"]).style(Style::new().dim())],
            columns(heading_area.width, app),
        )
        .column_spacing(1),
        heading_area,
    );
    draw_list(frame, app, list_area);
    draw_footer(frame, app, footer_area);
}

fn columns(width: u16, app: &App) -> [Constraint; 5] {
    let kind_width = app
        .agents
        .iter()
        .map(|agent| Line::from(agent.kind()).width())
        .max()
        .unwrap_or(5)
        .max(5);
    let kind_width = u16::try_from(kind_width).unwrap_or(u16::MAX).min(width / 5);
    let location_width = (0..app.agents.len())
        .map(|index| Line::from(app.location(index)).width())
        .max()
        .unwrap_or(9)
        .max(9);
    let location_width = u16::try_from(location_width)
        .unwrap_or(u16::MAX)
        .min(width / 4);
    [
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(location_width),
        Constraint::Length(kind_width),
    ]
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let line = match app.mode {
        Mode::Search => Line::from(vec![
            Span::styled("❯ ", Style::new().cyan().bold()),
            Span::raw(app.filter.as_str()),
        ]),
        Mode::Navigate => Line::from(vec![
            Span::styled("❯ ", Style::new().dim()),
            Span::styled("/ to filter", Style::new().dim().italic()),
        ]),
    };
    frame.render_widget(Paragraph::new(line), area);
    if app.mode == Mode::Search {
        let filter_width = u16::try_from(app.filter.chars().count()).unwrap_or(u16::MAX);
        frame.set_cursor_position(Position::new(
            area.x.saturating_add(2).saturating_add(filter_width),
            area.y,
        ));
    }
}

fn draw_list(frame: &mut Frame, app: &mut App, area: Rect) {
    app.visible_rows = 0..0;
    if area.is_empty() {
        return;
    }
    if app.agents.is_empty() {
        empty_state(frame, area, "No agent panes found", "r reload · q close");
        return;
    }
    if app.filtered.is_empty() {
        let message = format!("No matches for \"{}\"", app.filter);
        empty_state(frame, area, &message, "esc clears the filter");
        return;
    }

    let height = usize::from(area.height);
    let last = app.filtered.len() - 1;
    let selected = app.table_state.selected().map(|index| index.min(last));
    let mut start = app.table_state.offset().min(last);
    if let Some(selected) = selected {
        if selected < start {
            start = selected;
        } else if selected >= start + height {
            start = selected + 1 - height;
        }
    }
    let end = (start + height).min(app.filtered.len());
    if app.mode == Mode::Navigate {
        app.visible_rows = start..end;
    }
    app.table_state.select(selected);
    *app.table_state.offset_mut() = start;
    let mut table_state = TableState::new()
        .with_selected(selected.map(|index| index - start))
        .with_selected_column(app.table_state.selected_column());

    let tick = app.spinner_tick();
    let rows = app.filtered[start..end]
        .iter()
        .enumerate()
        .map(|(row, &i)| {
            let agent = &app.agents[i];
            let kind = agent.kind();
            let label = app.label(i);
            let location = app.location(i);

            let indices = app.highlights(start + row);
            let label_offset = kind.chars().count() + 1;
            let location_offset = label_offset + label.chars().count() + 1;

            let mut label_line = highlight_line(label, label_offset, indices, Style::new());
            if agent.focused {
                label_line.push_span(Span::styled(" (focused)", Style::new().dim()));
            }

            let shortcut = if app.mode == Mode::Navigate && row < 10 && agent.target().is_some() {
                ((row + 1) % 10).to_string()
            } else {
                String::new()
            };
            Row::new(vec![
                Cell::from(Span::styled(shortcut, Style::new().blue())),
                Cell::from(Span::styled(
                    status_glyph(agent.status(), tick),
                    status_style(agent.status()),
                )),
                Cell::from(label_line),
                Cell::from(highlight_line(
                    location,
                    location_offset,
                    indices,
                    Style::new().dim(),
                )),
                Cell::from(highlight_line(kind, 0, indices, Style::new().dim())),
            ])
        });

    let table = Table::new(rows, columns(area.width, app))
        .column_spacing(1)
        .row_highlight_style(selected_row_style());

    frame.render_stateful_widget(table, area, &mut table_state);
    app.table_state.select_column(table_state.selected_column());
}

fn selected_row_style() -> Style {
    Style::new().bg(Color::DarkGray)
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let line = match app.error.as_ref() {
        Some(error) => Line::from(vec![
            Span::styled(" ✗ ", Style::new().fg(Color::Red).bold()),
            Span::styled(error.as_str(), Style::new().fg(Color::Red)),
        ]),
        None => match app.mode {
            Mode::Navigate => hint_line(&[
                ("1-9/0", "open row"),
                ("enter", "focus"),
                ("j/k", "move"),
                ("/", "filter"),
                ("r", "reload"),
                ("q", "close"),
            ]),
            Mode::Search => hint_line(&[
                ("enter", "focus"),
                ("esc", "cancel"),
                ("↑/↓", "move"),
                ("^u", "clear"),
                ("^w", "delete word"),
            ]),
        },
    };
    frame.render_widget(Paragraph::new(line), area);
}

/// Key hints: keys at normal brightness, actions dimmed, so the keys are what
/// the eye lands on when scanning the footer.
fn hint_line(hints: &[(&'static str, &'static str)]) -> Line<'static> {
    let mut spans = Vec::with_capacity(hints.len() * 4 + 1);
    spans.push(Span::raw(" "));
    for (i, &(key, action)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::raw(key));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(action, Style::new().dim()));
    }
    Line::from(spans)
}

fn empty_state(frame: &mut Frame, area: Rect, message: &str, hint: &str) {
    let mut lines = vec![Line::raw(""); usize::from(area.height / 3)];
    lines.push(Line::from(message));
    if !hint.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(hint).dim());
    }
    frame.render_widget(Paragraph::new(lines).centered(), area);
}

/// Render `text` with fuzzy-matched chars emphasized. `offset` is where
/// `text` starts (in chars) inside the string `indices` was computed against.
/// Spans borrow byte ranges of `text`, so this never copies the string.
fn highlight_line<'a>(text: &'a str, offset: usize, indices: &[usize], base: Style) -> Line<'a> {
    if indices.is_empty() {
        return Line::from(Span::styled(text, base));
    }
    let matched_style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let style_for = |matched: bool| if matched { matched_style } else { base };

    let mut spans: Vec<Span<'a>> = Vec::new();
    let mut run_start = 0;
    let mut run_matched = false;
    for (i, (byte_start, _)) in text.char_indices().enumerate() {
        let matched = indices.binary_search(&(offset + i)).is_ok();
        if i == 0 {
            run_matched = matched;
        } else if matched != run_matched {
            spans.push(Span::styled(
                &text[run_start..byte_start],
                style_for(run_matched),
            ));
            run_start = byte_start;
            run_matched = matched;
        }
    }
    if !text.is_empty() {
        spans.push(Span::styled(&text[run_start..], style_for(run_matched)));
    }
    Line::from(spans)
}

/// Mirrors Herdr's built-in Agents sidebar `state_icon` token.
fn status_glyph(status: &str, tick: usize) -> &'static str {
    match status {
        "working" => SPINNER[tick % SPINNER.len()],
        "blocked" => "◉",
        "done" => "●",
        "idle" => "✓",
        _ => "○",
    }
}

fn status_style(status: &str) -> Style {
    let color = match status {
        "working" => Color::Yellow,
        "blocked" => Color::Red,
        "done" => Color::Cyan,
        "idle" => Color::Green,
        _ => Color::DarkGray,
    };
    Style::new().fg(color)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::herdr::parse_agent_list;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn sample_app() -> App {
        let agents = parse_agent_list(
            r#"{"result":{"agents":[
                {"agent":"claude","agent_status":"working","cwd":"/w/dotfiles","focused":true,
                 "pane_id":"w1:p1","terminal_id":"term_1","terminal_title_stripped":"dotfiles work"},
                {"agent":"codex","agent_status":"idle","cwd":"/w/dealon",
                 "pane_id":"w2:p1","terminal_id":"term_2","terminal_title_stripped":"dealon review"}
            ]}}"#,
        )
        .unwrap();
        let mut app = App::new(None, None);
        app.set_agents(agents);
        app
    }

    fn render(app: &mut App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let frame = terminal.draw(|frame| draw(frame, app)).unwrap();
        frame
            .buffer
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect()
    }

    fn numbered_app() -> App {
        let agents: Vec<_> = (0..8)
            .map(|index| {
                serde_json::json!({
                    "agent": "codex",
                    "agent_status": "idle",
                    "pane_id": format!("w1:p{index}"),
                    "terminal_title_stripped": format!("row-{index:02}")
                })
            })
            .collect();
        let response = serde_json::json!({"result": {"agents": agents}});
        let mut app = App::new(None, None);
        app.set_agents(parse_agent_list(&response.to_string()).unwrap());
        app
    }

    fn render_list(app: &mut App, height: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(100, height)).unwrap();
        let frame = terminal
            .draw(|frame| {
                let area = frame.area();
                draw_list(frame, app, area);
            })
            .unwrap();
        frame
            .buffer
            .content
            .chunks(100)
            .map(|row| {
                let line: String = row.iter().map(ratatui::buffer::Cell::symbol).collect();
                line.split_whitespace()
                    .find(|word| word.starts_with("row-"))
                    .unwrap_or("")
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn list_scrolls_and_wraps_with_the_selected_agent() {
        let mut app = numbered_app();
        assert_eq!(render_list(&mut app, 3), ["row-00", "row-01", "row-02"]);
        for _ in 0..4 {
            app.move_selection(1);
        }
        assert_eq!(render_list(&mut app, 3), ["row-02", "row-03", "row-04"]);
        assert_eq!(app.table_state.offset(), 2);
        for _ in 0..4 {
            app.move_selection(1);
        }
        assert_eq!(render_list(&mut app, 3), ["row-00", "row-01", "row-02"]);
        app.move_selection(-1);
        assert_eq!(render_list(&mut app, 3), ["row-05", "row-06", "row-07"]);
        assert_eq!(app.table_state.selected(), Some(7));
        assert_eq!(app.table_state.offset(), 5);
    }

    #[test]
    fn resizing_keeps_the_offset_until_selection_requires_scrolling() {
        let mut app = numbered_app();
        app.table_state.select(Some(7));
        assert_eq!(render_list(&mut app, 3), ["row-05", "row-06", "row-07"]);
        assert_eq!(
            render_list(&mut app, 5),
            ["row-05", "row-06", "row-07", "", ""]
        );
        assert_eq!(render_list(&mut app, 2), ["row-06", "row-07"]);
        app.move_selection(-1);
        app.move_selection(-1);
        assert_eq!(render_list(&mut app, 2), ["row-05", "row-06"]);
        assert_eq!(app.table_state.offset(), 5);
    }

    #[test]
    fn empty_list_area_preserves_selection_and_offset() {
        let mut app = numbered_app();
        app.table_state = TableState::new()
            .with_selected(Some(99))
            .with_offset(99)
            .with_selected_column(Some(99));
        let state = app.table_state.clone();
        let mut terminal = Terminal::new(TestBackend::new(100, 1)).unwrap();
        for area in [Rect::new(0, 0, 0, 1), Rect::new(0, 0, 100, 0)] {
            terminal
                .draw(|frame| draw_list(frame, &mut app, area))
                .unwrap();
            assert_eq!(app.table_state, state);
        }
    }

    #[test]
    fn filtering_clamps_the_viewport_to_the_remaining_agent() {
        let mut app = numbered_app();
        app.table_state.select(Some(7));
        render_list(&mut app, 3);
        app.enter_search();
        for character in "row-02".chars() {
            app.push_char(character);
        }
        assert_eq!(render_list(&mut app, 3), ["row-02", "", ""]);
        assert_eq!(app.table_state.selected(), Some(0));
        assert_eq!(app.table_state.offset(), 0);
    }

    #[test]
    fn viewport_preserves_no_selection_and_clamps_selected_columns() {
        let mut app = numbered_app();
        app.table_state = TableState::new()
            .with_offset(6)
            .with_selected_column(Some(99));
        assert_eq!(render_list(&mut app, 3), ["row-06", "row-07", ""]);
        assert_eq!(app.table_state.selected(), None);
        assert_eq!(app.table_state.offset(), 6);
        assert_eq!(app.table_state.selected_column(), Some(4));
        app.table_state.select(Some(99));
        assert_eq!(render_list(&mut app, 3), ["row-06", "row-07", ""]);
        assert_eq!(app.table_state.selected(), Some(7));
    }

    #[test]
    fn scrolled_rows_use_their_own_search_highlights() {
        let agents = parse_agent_list(
            r#"{"result":{"agents":[
                {"agent":"codex","terminal_title_stripped":"long row-first"},
                {"agent":"codex","terminal_title_stripped":"row-second"}
            ]}}"#,
        )
        .unwrap();
        let mut app = App::new(None, None);
        app.set_agents(agents);
        app.enter_search();
        for character in "row".chars() {
            app.push_char(character);
        }
        let row = app
            .filtered
            .iter()
            .position(|&index| app.label(index) == "long row-first")
            .unwrap();
        assert_eq!(row, 1);
        app.table_state.select(Some(row));
        let mut terminal = Terminal::new(TestBackend::new(100, 1)).unwrap();
        let frame = terminal
            .draw(|frame| {
                let area = frame.area();
                draw_list(frame, &mut app, area);
            })
            .unwrap();
        let cells = &frame.buffer.content;
        let text: String = cells.iter().map(ratatui::buffer::Cell::symbol).collect();
        let label_start = text.find("long row-first").unwrap();
        let column = text[..label_start].chars().count();
        assert_ne!(cells[column].fg, Color::Cyan);
        assert_eq!(
            [
                cells[column + 5].fg,
                cells[column + 6].fg,
                cells[column + 7].fg
            ],
            [Color::Cyan; 3]
        );
    }

    #[test]
    fn numbers_label_visible_rows_and_disappear_during_search() {
        let mut app = numbered_app();
        let screen = render(&mut app, 80, 6);
        assert!(screen.contains("1 ✓ row-00"));
        assert!(screen.contains("3 ✓ row-02"));
        assert!(!screen.contains("4 ✓"));
        app.move_selection(-1);
        let screen = render(&mut app, 80, 6);
        assert!(screen.contains("1 ✓ row-05"));
        assert!(screen.contains("3 ✓ row-07"));
        app.enter_search();
        assert!(!render(&mut app, 80, 6).contains("1 ✓"));
    }

    #[test]
    fn explicit_thread_names_replace_automatic_titles() {
        let mut app = App::new(None, None);
        app.set_agents(parse_agent_list(
            r#"{"result":{"agents":[{"agent":"pi","name":"test","pane_id":"w1:p1","terminal_title_stripped":"π - old automatic title"}]}}"#,
        ).unwrap());
        let screen = render(&mut app, 100, 8);
        assert!(screen.contains("test"));
        assert!(!screen.contains("old automatic title"));
        assert!(!screen.contains("π -"));
    }

    #[test]
    fn thread_titles_use_the_full_popup_without_an_inner_frame() {
        let mut app = App::new(None, None);
        app.set_agents(parse_agent_list(
            r#"{"result":{"agents":[{"agent":"codex","agent_status":"idle","pane_id":"w1:p1","terminal_title_stripped":"Fix roster eligibility after season rollover"}]}}"#,
        ).unwrap());
        let screen = render(&mut app, 80, 12);
        assert!(screen.contains("Fix roster eligibility after season rollover"));
        assert!(!screen.contains("Agents"));
        assert!(!screen.chars().any(|c| "│─╭╮╰╯┌┐└┘".contains(c)));
        assert!(screen.contains("1/1"));
    }

    #[test]
    fn draws_the_agent_list_with_status_and_count() {
        let mut app = sample_app();
        let screen = render(&mut app, 200, 20);
        assert!(screen.contains("2/2"));
        assert!(screen.contains("dotfiles work"));
        assert!(screen.contains("(focused)"));
        assert!(screen.contains("✓")); // idle codex
    }

    #[test]
    fn status_icons_match_the_builtin_agents_sidebar() {
        assert_eq!(status_glyph("working", 0), "⠋");
        assert_eq!(status_glyph("blocked", 0), "◉");
        assert_eq!(status_glyph("done", 0), "●");
        assert_eq!(status_glyph("idle", 0), "✓");
        assert_eq!(status_glyph("unknown", 0), "○");
    }

    #[test]
    fn status_colors_match_the_builtin_agents_sidebar() {
        assert_eq!(status_style("working").fg, Some(Color::Yellow));
        assert_eq!(status_style("blocked").fg, Some(Color::Red));
        assert_eq!(status_style("done").fg, Some(Color::Cyan));
        assert_eq!(status_style("idle").fg, Some(Color::Green));
        assert_eq!(status_style("unknown").fg, Some(Color::DarkGray));
    }

    #[test]
    fn selected_row_uses_a_subtle_background_without_reversing_colors() {
        let style = selected_row_style();

        assert_eq!(
            (style.bg, style.add_modifier),
            (Some(Color::DarkGray), Modifier::empty())
        );
    }

    #[test]
    fn draws_empty_and_no_match_states() {
        let mut app = App::new(None, None);
        let screen = render(&mut app, 100, 20);
        assert!(screen.contains("No agent panes found"));

        let mut app = sample_app();
        app.enter_search();
        for c in "zzz".chars() {
            app.push_char(c);
        }
        let screen = render(&mut app, 100, 20);
        assert!(screen.contains("No matches for \"zzz\""));
    }

    #[test]
    fn search_mode_shows_the_filter_and_hints() {
        let mut app = sample_app();
        app.enter_search();
        for c in "deal".chars() {
            app.push_char(c);
        }
        let screen = render(&mut app, 100, 20);
        assert!(screen.contains("❯ deal"));
        assert!(screen.contains("1/2"));
        assert!(screen.contains("esc cancel"));
    }

    #[test]
    fn survives_tiny_terminals() {
        let mut app = sample_app();
        render(&mut app, 8, 3);
        render(&mut app, 1, 1);
    }

    #[test]
    fn list_error_is_visible_in_the_footer() {
        let mut app = sample_app();
        app.error = Some("agent list failed: unsupported command".to_string());

        assert!(render(&mut app, 120, 20).contains("unsupported command"));
    }
}
