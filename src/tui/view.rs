use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Widget, Wrap};

use crate::doc::{Case, Doc, Group, DECISION_COLUMN};

use super::state::App;

const OVERLAY_PERCENT_X: u16 = 90;
const OVERLAY_PERCENT_Y: u16 = 90;

const NUM_COL_WIDTH: u16 = 12;
const STATUS_COL_WIDTH: u16 = 15;
const SUBMIT_WIDTH: u16 = 30;
const SUBMIT_HEIGHT: u16 = 3;

const COLOR_BORDER_FADED: Color = Color::DarkGray;
const COLOR_BORDER_SELECTED: Color = Color::Cyan;
const COLOR_TEXT_FILLED: Color = Color::White;
const COLOR_TEXT_EMPTY: Color = Color::DarkGray;
const COLOR_HEADER: Color = Color::Yellow;
const COLOR_MARKER: Color = Color::Rgb(255, 230, 80);

pub fn render(frame: &mut Frame, app: &mut App) {
    let frame_area = frame.area();

    // If we have a captured backdrop (--popup mode), draw it across the full
    // frame (already dimmed at capture time in tui::run), then carve out a
    // centered overlay for the dialog. Otherwise the dialog takes the whole
    // frame.
    let dialog_area = if let Some(backdrop) = app.backdrop.clone() {
        frame.render_widget(Paragraph::new(backdrop), frame_area);

        let overlay = centered_rect(frame_area, OVERLAY_PERCENT_X, OVERLAY_PERCENT_Y);
        frame.render_widget(Clear, overlay);
        overlay
    } else {
        frame_area
    };

    render_dialog(frame, app, dialog_area);
}

fn render_dialog(frame: &mut Frame, app: &mut App, area: Rect) {
    let outer = Block::bordered()
        .border_type(BorderType::Thick)
        .padding(Padding::new(2, 2, 1, 1));
    let inner_area = outer.inner(area);
    frame.render_widget(outer, area);

    let [header_area, body_area, footer_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(inner_area);

    render_header(frame, header_area, app);
    render_footer(frame, footer_area);
    render_body(frame, body_area, app);
}

fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let header_block = Block::default().borders(Borders::BOTTOM);
    let inner = header_block.inner(area);
    frame.render_widget(header_block, area);

    let topic = app
        .doc
        .frontmatter
        .topic
        .as_deref()
        .unwrap_or("(no topic)");

    let total = app.total_cases();
    let filled = app.filled_cases();
    let pct = if total == 0 {
        0
    } else {
        (filled * 100) / total
    };

    let bar_width = 10usize;
    let filled_bar = (pct * bar_width) / 100;
    let bar: String = "█".repeat(filled_bar) + &"░".repeat(bar_width - filled_bar);

    let warn_str = if app.warnings.is_empty() {
        String::new()
    } else {
        format!("⚠ {}   ", app.warnings.len())
    };

    let progress_str = format!("{}[{}] {} / {}", warn_str, bar, filled, total);
    let progress_width = (progress_str.chars().count() as u16) + 2;

    let [topic_area, progress_area] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(progress_width)])
            .areas(inner);

    let topic_para = Paragraph::new(topic).style(Style::new().add_modifier(Modifier::BOLD));
    frame.render_widget(topic_para, topic_area);

    let progress_para = Paragraph::new(progress_str).alignment(Alignment::Right);
    frame.render_widget(progress_para, progress_area);
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let text = "↑↓ navigate · ⏎/⇥ next · ⇧⇥ prev · ⌫ delete · Submit closes";
    let footer = Paragraph::new(text)
        .alignment(Alignment::Center)
        .style(Style::new().fg(Color::DarkGray));
    frame.render_widget(footer, area);
}

fn render_body(frame: &mut Frame, area: Rect, app: &mut App) {
    if area.width < 10 || area.height < 3 {
        return;
    }

    let plan = compute_layout(&app.doc, area.width);

    if app.cursor == 0 {
        // Snap to top so the first group's header and description are visible.
        app.scroll = 0;
    } else if app.cursor < plan.cursor_positions.len() {
        let pos = plan.cursor_positions[app.cursor];
        let eighth = area.height / 8;

        if pos.top < app.scroll + eighth && app.scroll > 0 {
            // In upper eighth (or above viewport) — scroll to center.
            let mid = (pos.top + pos.bottom) / 2;
            app.scroll = mid.saturating_sub(area.height / 2);
        } else if pos.bottom > app.scroll + area.height.saturating_sub(eighth) {
            // In lower eighth (or below viewport) — scroll to center.
            let mid = (pos.top + pos.bottom) / 2;
            app.scroll = mid.saturating_sub(area.height / 2);
        }
    }

    let max_scroll = plan.total_height.saturating_sub(area.height);
    if app.scroll > max_scroll {
        app.scroll = max_scroll;
    }

    let buf_height = plan.total_height.max(SUBMIT_HEIGHT);
    let mut tall = Buffer::empty(Rect::new(0, 0, area.width, buf_height));

    render_to_tall_buffer(&mut tall, app, &plan);

    blit(&tall, app.scroll, frame.buffer_mut(), area);
}

#[derive(Default)]
struct LayoutPlan {
    total_height: u16,
    /// y range of each selectable item (status cells in order, then Submit).
    cursor_positions: Vec<CursorRect>,
    /// Per-group computed metrics so render can reuse them.
    groups: Vec<GroupLayout>,
    /// y position where the Submit button starts.
    submit_y: u16,
}

#[derive(Clone)]
struct GroupLayout {
    /// y where the group's content starts in the body coordinate space.
    y: u16,
    header_h: u16,
    description_h: u16,
    table_y: u16,
    table_h: u16,
    row_height: u16,
    column_widths: Vec<u16>,
}

#[derive(Clone, Copy)]
struct CursorRect {
    top: u16,
    bottom: u16,
}

fn compute_layout(doc: &Doc, width: u16) -> LayoutPlan {
    let mut y = 0u16;
    let mut positions = Vec::new();
    let mut groups = Vec::new();

    // Column widths and row height are computed globally so every table renders
    // with the same dimensions, regardless of which group's cases happen to need
    // the most vertical space.
    let column_widths = compute_column_widths(width, &doc.frontmatter.columns);
    let row_height = compute_global_row_height(&doc.groups, &doc.frontmatter.columns, &column_widths);

    for group in &doc.groups {
        let group_y = y;

        let header_h = if group.name.is_some() { 2 } else { 0 };
        y += header_h;

        let description_h = if group.description.is_empty() {
            0
        } else {
            estimate_para_height(&group.description, width) + 1
        };
        y += description_h;

        // Table layout
        let table_y = y;
        let header_strip = 2u16; // header line + separator
        let table_h = 2 + header_strip + (group.cases.len() as u16) * row_height; // 2 for outer borders
        y += table_h;

        // Cursor positions for each case row (in body coords)
        let row_start_y = table_y + 1 + header_strip; // +1 for outer top border
        for ci in 0..group.cases.len() {
            let top = row_start_y + (ci as u16) * row_height;
            let bottom = top + row_height;
            positions.push(CursorRect { top, bottom });
        }

        y += 1; // group spacing

        groups.push(GroupLayout {
            y: group_y,
            header_h,
            description_h,
            table_y,
            table_h,
            row_height,
            column_widths: column_widths.clone(),
        });
    }

    let submit_y = y + 1;
    y = submit_y + SUBMIT_HEIGHT;
    positions.push(CursorRect {
        top: submit_y,
        bottom: submit_y + SUBMIT_HEIGHT,
    });

    LayoutPlan {
        total_height: y,
        cursor_positions: positions,
        groups,
        submit_y,
    }
}

fn compute_column_widths(table_width: u16, columns: &[String]) -> Vec<u16> {
    let inner_width = table_width.saturating_sub(2); // outer table borders

    let constraints: Vec<Constraint> = std::iter::once(Constraint::Length(NUM_COL_WIDTH))
        .chain(columns.iter().map(|c| {
            if c.eq_ignore_ascii_case(DECISION_COLUMN) {
                Constraint::Length(STATUS_COL_WIDTH)
            } else {
                Constraint::Fill(1)
            }
        }))
        .collect();

    let dummy = Rect::new(0, 0, inner_width, 1);
    let areas = Layout::horizontal(constraints).split(dummy);
    areas.iter().map(|r| r.width).collect()
}

fn compute_global_row_height(groups: &[Group], columns: &[String], widths: &[u16]) -> u16 {
    let mut max_h = 3u16; // minimum: 1 char top padding + 1 line text + 1 char bottom padding

    for group in groups {
        for case in &group.cases {
            let num_inner = widths[0].saturating_sub(2);
            let num_text = format!("#{} {}", case.number, case.name);
            let h = wrap_height(&num_text, num_inner) + 2;
            max_h = max_h.max(h);

            for (i, col) in columns.iter().enumerate() {
                let value = case.fields.get(col).cloned().unwrap_or_default();
                let inner = widths[i + 1].saturating_sub(2);
                let h = wrap_height(&value, inner) + 2;
                max_h = max_h.max(h);
            }
        }
    }

    max_h
}

fn estimate_para_height(text: &str, width: u16) -> u16 {
    if text.is_empty() || width == 0 {
        return 0;
    }
    let mut total = 0u16;
    for line in text.lines() {
        let chars = line.chars().count() as u16;
        total += if chars == 0 { 1 } else { chars.div_ceil(width) };
    }
    total.max(1)
}

fn wrap_height(text: &str, width: u16) -> u16 {
    if width == 0 {
        return 1;
    }
    if text.is_empty() {
        return 1;
    }
    let mut total = 0u16;
    for line in text.lines() {
        let chars = line.chars().count() as u16;
        total += if chars == 0 { 1 } else { chars.div_ceil(width) };
    }
    total.max(1)
}

fn render_to_tall_buffer(buf: &mut Buffer, app: &App, plan: &LayoutPlan) {
    let width = buf.area().width;
    let selection = app.selected_case().map(|case| CellSelection {
        case,
        cursor_pos: app.input_cursor,
    });

    for (gi, group) in app.doc.groups.iter().enumerate() {
        let layout = &plan.groups[gi];
        let mut y = layout.y;

        if let Some(name) = &group.name {
            Paragraph::new(name.as_str())
                .style(
                    Style::new()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::Cyan),
                )
                .render(Rect::new(0, y, width, 1), buf);
            y += layout.header_h;
        }

        if !group.description.is_empty() {
            let h = layout.description_h.saturating_sub(1);
            Paragraph::new(group.description.as_str())
                .wrap(Wrap { trim: false })
                .style(Style::new().fg(Color::Gray))
                .render(Rect::new(0, y, width, h), buf);
            y += layout.description_h;
        }

        debug_assert_eq!(y, layout.table_y);
        render_table(buf, layout.table_y, width, group, gi, &app.doc.frontmatter.columns, layout, selection);
    }

    render_submit_button(buf, plan.submit_y, width, app.is_on_submit());
}

#[derive(Clone, Copy)]
struct CellSelection {
    case: (usize, usize),
    cursor_pos: usize,
}

fn render_table(
    buf: &mut Buffer,
    base_y: u16,
    width: u16,
    group: &Group,
    group_idx: usize,
    columns: &[String],
    layout: &GroupLayout,
    selection: Option<CellSelection>,
) {
    // Outer table border
    let table_area = Rect::new(0, base_y, width, layout.table_h);
    Block::bordered()
        .border_style(Style::new().fg(COLOR_BORDER_FADED))
        .render(table_area, buf);

    let inner_x = 1u16;
    let inner_y = base_y + 1;
    let inner_width = width.saturating_sub(2);

    // Header row
    let mut hx = inner_x;
    render_header_cell(buf, hx, inner_y, layout.column_widths[0], "#");
    hx += layout.column_widths[0];
    for (i, col) in columns.iter().enumerate() {
        render_header_cell(buf, hx, inner_y, layout.column_widths[i + 1], col);
        hx += layout.column_widths[i + 1];
    }

    // Header separator line
    let sep_y = inner_y + 1;
    for sx in inner_x..inner_x + inner_width {
        if let Some(cell) = buf.cell_mut(Position::new(sx, sep_y)) {
            cell.set_char('─').set_style(Style::new().fg(COLOR_BORDER_FADED));
        }
    }

    // Data rows
    let row_start_y = inner_y + 2;
    for (ci, case) in group.cases.iter().enumerate() {
        let row_y = row_start_y + (ci as u16) * layout.row_height;
        render_case_row(
            buf,
            inner_x,
            row_y,
            layout.row_height,
            &layout.column_widths,
            case,
            columns,
            group_idx,
            ci,
            selection,
        );
    }
}

fn render_header_cell(buf: &mut Buffer, x: u16, y: u16, width: u16, text: &str) {
    let inner = Rect::new(x + 1, y, width.saturating_sub(2), 1);
    Paragraph::new(text)
        .style(
            Style::new()
                .add_modifier(Modifier::BOLD)
                .fg(COLOR_HEADER),
        )
        .render(inner, buf);
}

fn render_case_row(
    buf: &mut Buffer,
    base_x: u16,
    base_y: u16,
    height: u16,
    widths: &[u16],
    case: &Case,
    columns: &[String],
    group_idx: usize,
    case_idx: usize,
    selection: Option<CellSelection>,
) {
    let mut cx = base_x;

    // # column
    let num_text = format!("#{} {}", case.number, case.name);
    render_text_cell(
        buf,
        Rect::new(cx, base_y, widths[0], height),
        Text::from(num_text),
        Style::new().fg(COLOR_TEXT_FILLED),
    );
    cx += widths[0];

    for (i, col) in columns.iter().enumerate() {
        let value = case.fields.get(col).cloned().unwrap_or_default();
        let is_status = col.eq_ignore_ascii_case(DECISION_COLUMN);
        let is_selected = is_status
            && selection
                .map(|s| s.case == (group_idx, case_idx))
                .unwrap_or(false);
        let is_filled = !value.trim().is_empty();

        let base_style = if is_filled {
            Style::new().fg(COLOR_TEXT_FILLED)
        } else {
            Style::new().fg(COLOR_TEXT_EMPTY)
        };

        let cell_text = if is_status {
            let cursor_pos = if is_selected {
                selection.map(|s| s.cursor_pos.min(value.len()))
            } else {
                None
            };
            build_status_text(&value, cursor_pos)
        } else {
            Text::from(value.clone())
        };

        let cell_area = Rect::new(cx, base_y, widths[i + 1], height);
        render_text_cell(buf, cell_area, cell_text, base_style);

        if is_selected {
            draw_thick_border(
                buf,
                cell_area,
                Style::new()
                    .fg(COLOR_BORDER_SELECTED)
                    .add_modifier(Modifier::BOLD),
            );
        }

        cx += widths[i + 1];
    }
}

/// Build a Text for a Status cell. The leading marker token (`!`, `!!`, `?`,
/// `OK`) is rendered in marker yellow; everything else inherits the cell's
/// base style. If `cursor_pos` is `Some`, a block cursor is rendered at that
/// byte offset using the inverted-text-on-overlap convention: the character
/// under the cursor is rendered with `Modifier::REVERSED` (swapping fg/bg). If
/// the cursor sits past the last character, an extra `█` block is appended.
fn build_status_text<'a>(value: &str, cursor_pos: Option<usize>) -> Text<'a> {
    let segments = segment_value(value);
    let mut spans: Vec<Span<'static>> = Vec::new();

    let make_span = |text: String, style: Option<Style>| -> Span<'static> {
        match style {
            Some(s) => Span::styled(text, s),
            None => Span::raw(text),
        }
    };

    let mut consumed = 0usize;
    let mut cursor_emitted = cursor_pos.is_none();

    for seg in segments {
        let seg_len = seg.text.len();
        let seg_chars: Vec<(usize, char)> = seg.text.char_indices().collect();
        let mut emitted_through = 0usize;

        for (i, (off, ch)) in seg_chars.iter().enumerate() {
            let abs = consumed + off;
            let cursor_here = match cursor_pos {
                Some(cpos) if !cursor_emitted && cpos == abs => true,
                _ => false,
            };
            if cursor_here {
                if emitted_through < *off {
                    spans.push(make_span(
                        seg.text[emitted_through..*off].to_string(),
                        seg.style,
                    ));
                }
                let inverted = inverted_style(seg.style);
                spans.push(Span::styled(ch.to_string(), inverted));
                emitted_through = off + ch.len_utf8();
                cursor_emitted = true;
                let _ = i;
            }
        }

        if emitted_through < seg_len {
            spans.push(make_span(
                seg.text[emitted_through..].to_string(),
                seg.style,
            ));
        }

        consumed += seg_len;
    }

    if let Some(cpos) = cursor_pos
        && !cursor_emitted
        && cpos >= consumed
    {
        spans.push(Span::styled(
            "█".to_string(),
            Style::new().fg(COLOR_TEXT_FILLED),
        ));
    }

    Text::from(Line::from(spans))
}

fn inverted_style(base: Option<Style>) -> Style {
    let style = base.unwrap_or_else(|| Style::new().fg(COLOR_TEXT_FILLED));
    style.add_modifier(Modifier::REVERSED)
}

struct StatusSegment {
    text: String,
    style: Option<Style>,
}

fn segment_value(value: &str) -> Vec<StatusSegment> {
    let mut segments = Vec::new();

    let trimmed_start = value.trim_start();
    let leading_len = value.len() - trimmed_start.len();
    if leading_len > 0 {
        segments.push(StatusSegment {
            text: value[..leading_len].to_string(),
            style: None,
        });
    }

    if trimmed_start.is_empty() {
        return segments;
    }

    let first_token_end = trimmed_start
        .find(char::is_whitespace)
        .unwrap_or(trimmed_start.len());
    let first_token = &trimmed_start[..first_token_end];

    let marker_style = if matches!(first_token, "!" | "!!" | "?" | "OK") {
        Some(Style::new().fg(COLOR_MARKER))
    } else if first_token.eq_ignore_ascii_case("skip") {
        Some(Style::new().fg(Color::DarkGray))
    } else {
        None
    };

    if let Some(style) = marker_style {
        segments.push(StatusSegment {
            text: first_token.to_string(),
            style: Some(style),
        });
        let rest = &trimmed_start[first_token_end..];
        if !rest.is_empty() {
            segments.push(StatusSegment {
                text: rest.to_string(),
                style: None,
            });
        }
    } else {
        segments.push(StatusSegment {
            text: trimmed_start.to_string(),
            style: None,
        });
    }

    segments
}

fn render_text_cell(buf: &mut Buffer, area: Rect, text: Text<'_>, style: Style) {
    let inner = Rect::new(
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .style(style)
        .render(inner, buf);
}

fn draw_thick_border(buf: &mut Buffer, area: Rect, style: Style) {
    if area.width < 2 || area.height < 2 {
        return;
    }
    let r = area.x + area.width - 1;
    let b = area.y + area.height - 1;

    for &(x, y, ch) in &[
        (area.x, area.y, '┏'),
        (r, area.y, '┓'),
        (area.x, b, '┗'),
        (r, b, '┛'),
    ] {
        if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
            cell.set_char(ch).set_style(style);
        }
    }

    for x in area.x + 1..r {
        if let Some(cell) = buf.cell_mut(Position::new(x, area.y)) {
            cell.set_char('━').set_style(style);
        }
        if let Some(cell) = buf.cell_mut(Position::new(x, b)) {
            cell.set_char('━').set_style(style);
        }
    }

    for y in area.y + 1..b {
        if let Some(cell) = buf.cell_mut(Position::new(area.x, y)) {
            cell.set_char('┃').set_style(style);
        }
        if let Some(cell) = buf.cell_mut(Position::new(r, y)) {
            cell.set_char('┃').set_style(style);
        }
    }
}

fn render_submit_button(buf: &mut Buffer, y: u16, width: u16, selected: bool) {
    let border_style = if selected {
        Style::new()
            .fg(COLOR_BORDER_SELECTED)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(COLOR_BORDER_FADED)
    };
    let text_style = if selected {
        Style::new().fg(COLOR_MARKER)
    } else {
        Style::new().fg(COLOR_TEXT_FILLED)
    };

    let x = width.saturating_sub(SUBMIT_WIDTH) / 2;
    let area = Rect::new(x, y, SUBMIT_WIDTH, SUBMIT_HEIGHT);

    let block = Block::bordered().border_style(border_style);
    let inner = block.inner(area);
    block.render(area, buf);

    Paragraph::new("Submit")
        .alignment(Alignment::Center)
        .style(text_style)
        .render(inner, buf);
}

fn blit(src: &Buffer, src_y: u16, dst: &mut Buffer, dst_area: Rect) {
    let src_area = src.area();
    let h = dst_area.height.min(src_area.height.saturating_sub(src_y));
    let w = dst_area.width.min(src_area.width);
    for dy in 0..h {
        for dx in 0..w {
            let src_pos = Position::new(dx, src_y + dy);
            let dst_pos = Position::new(dst_area.x + dx, dst_area.y + dy);
            if let Some(src_cell) = src.cell(src_pos)
                && let Some(dst_cell) = dst.cell_mut(dst_pos)
            {
                *dst_cell = src_cell.clone();
            }
        }
    }
}
