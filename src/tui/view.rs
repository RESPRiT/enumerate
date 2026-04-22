use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Widget, Wrap};

use crate::doc::{Case, Doc, Group, DECISION_COLUMN};

use super::state::App;

const MARGIN_BREAKPOINTS: [(u16, u16); 4] = [
    (60, 3),
    (40, 2),
    (20, 1),
    (0, 0),
];

const NUM_COL_WIDTH: u16 = 16;
const STATUS_COL_WIDTH: u16 = 20;
const SUBMIT_WIDTH: u16 = 30;
const SUBMIT_HEIGHT: u16 = 3;
const MAX_TEXT_LINES: u16 = 3;
const MAX_ROW_HEIGHT: u16 = MAX_TEXT_LINES + 2; // 1 pad + 3 text + 1 pad

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

        let overlay = margin_rect(frame_area);
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

fn margin_rect(area: Rect) -> Rect {
    let smaller = area.width.min(area.height);
    let margin_y = MARGIN_BREAKPOINTS
        .iter()
        .find(|(threshold, _)| smaller >= *threshold)
        .map(|(_, m)| *m)
        .unwrap_or(0);
    let margin_x = margin_y * 2;

    Rect {
        x: area.x + margin_x,
        y: area.y + margin_y,
        width: area.width.saturating_sub(margin_x * 2),
        height: area.height.saturating_sub(margin_y * 2),
    }
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

    let plan = compute_layout(&app.doc, area.width, app.selected_case(), app.input_cursor);

    let expand_extra = plan.expanded.as_ref()
        .map(|e| e.height.saturating_sub(plan.groups[e.group_idx].row_height))
        .unwrap_or(0);
    let content_height = plan.total_height + expand_extra;

    if app.cursor == 0 {
        // Snap to top so the first group's header and description are visible.
        app.scroll = 0;
    } else if app.cursor < plan.cursor_positions.len() {
        let pos = plan.cursor_positions[app.cursor];
        let need_top = pos.top;
        let mut need_bottom = pos.bottom;

        // When expanded, extend the scroll target to include the full
        // expansion height + the input cursor position, plus a small
        // padding so the expansion doesn't press against the viewport edge.
        if let Some(ref expanded) = plan.expanded {
            // Include full expansion height.
            need_bottom = need_bottom.max(expanded.y + expanded.height + 2);

            // Also track the input cursor within the Decision cell.
            let (gi, ci) = (expanded.group_idx, expanded.case_idx);
            let value = app.doc.groups[gi].cases[ci]
                .fields
                .get(DECISION_COLUMN)
                .map(String::as_str)
                .unwrap_or("");
            let col_idx = app
                .doc
                .frontmatter
                .columns
                .iter()
                .position(|c| c.eq_ignore_ascii_case(DECISION_COLUMN))
                .unwrap_or(0);
            let inner_w = plan.groups[gi].column_widths[col_idx + 1].saturating_sub(2);
            let prefix = &value[..app.input_cursor.min(value.len())];
            let cursor_line = if inner_w == 0 || prefix.is_empty() {
                0
            } else if app.input_cursor >= value.len() {
                // Cursor is past the end — the █ block is an extra char.
                wrap_height_with_cursor(prefix, inner_w).saturating_sub(1)
            } else {
                wrap_height(prefix, inner_w).saturating_sub(1)
            };
            let cursor_y = expanded.y + 1 + cursor_line;
            need_bottom = need_bottom.max(cursor_y + 3);
        }

        let range = need_bottom.saturating_sub(need_top);
        let twelfth = area.height / 12;

        if range > area.height {
            // Target taller than viewport — keep cursor visible, sacrifice top.
            app.scroll = need_bottom.saturating_sub(area.height);
        } else if need_top < app.scroll || need_bottom > app.scroll + area.height {
            // Off-screen — center.
            let mid = (need_top + need_bottom) / 2;
            app.scroll = mid.saturating_sub(area.height / 2);
        } else if need_top < app.scroll + twelfth && app.scroll > 0 {
            // Upper eighth — center.
            let mid = (need_top + need_bottom) / 2;
            app.scroll = mid.saturating_sub(area.height / 2);
        } else if need_bottom > app.scroll + area.height.saturating_sub(twelfth) {
            // Lower eighth — center.
            let mid = (need_top + need_bottom) / 2;
            app.scroll = mid.saturating_sub(area.height / 2);
        }
    }

    let max_scroll = content_height.saturating_sub(area.height);
    if app.scroll > max_scroll {
        app.scroll = max_scroll;
    }

    let buf_height = content_height.max(SUBMIT_HEIGHT);
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
    /// If the selected row needs more than the capped height, stores expansion info.
    expanded: Option<ExpandedRow>,
}

struct ExpandedRow {
    group_idx: usize,
    case_idx: usize,
    /// Y position of the row in the tall buffer.
    y: u16,
    /// Natural (uncapped) height of the row.
    height: u16,
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

fn compute_layout(doc: &Doc, width: u16, selection: Option<(usize, usize)>, input_cursor: usize) -> LayoutPlan {
    let mut y = 0u16;
    let mut positions = Vec::new();
    let mut groups = Vec::new();
    let mut expanded = None;

    let column_widths = compute_column_widths(width, &doc.frontmatter.columns);
    let row_height = compute_global_row_height(&doc.groups, &doc.frontmatter.columns, &column_widths)
        .min(MAX_ROW_HEIGHT);

    for (gi, group) in doc.groups.iter().enumerate() {
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
        for (ci, case) in group.cases.iter().enumerate() {
            let top = row_start_y + (ci as u16) * row_height;
            let bottom = top + row_height;
            positions.push(CursorRect { top, bottom });

            if selection == Some((gi, ci)) {
                let mut natural_h = compute_case_row_height(case, &doc.frontmatter.columns, &column_widths);
                // The █ cursor block at the end of the Decision field is an
                // extra character that can wrap to a new line.
                let decision = case.fields.get(DECISION_COLUMN).map(String::as_str).unwrap_or("");
                if input_cursor >= decision.len() {
                    let col_idx = doc.frontmatter.columns.iter()
                        .position(|c| c.eq_ignore_ascii_case(DECISION_COLUMN))
                        .unwrap_or(0);
                    let inner_w = column_widths[col_idx + 1].saturating_sub(2);
                    let cursor_h = wrap_height_with_cursor(decision, inner_w) + 2;
                    natural_h = natural_h.max(cursor_h);
                }
                if natural_h > row_height {
                    expanded = Some(ExpandedRow {
                        group_idx: gi,
                        case_idx: ci,
                        y: top,
                        height: natural_h,
                    });
                }
            }
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
        expanded,
    }
}

fn compute_column_widths(table_width: u16, columns: &[String]) -> Vec<u16> {
    let inner_width = table_width.saturating_sub(2); // outer table borders

    // The first non-Decision column (typically Description) gets a larger
    // share of the remaining space than subsequent columns (e.g. Notes).
    let mut first_fill = true;
    let constraints: Vec<Constraint> = std::iter::once(Constraint::Length(NUM_COL_WIDTH))
        .chain(columns.iter().map(|c| {
            if c.eq_ignore_ascii_case(DECISION_COLUMN) {
                Constraint::Length(STATUS_COL_WIDTH)
            } else if first_fill {
                first_fill = false;
                Constraint::Fill(3)
            } else {
                Constraint::Fill(2)
            }
        }))
        .collect();

    let dummy = Rect::new(0, 0, inner_width, 1);
    let areas = Layout::horizontal(constraints).split(dummy);
    areas.iter().map(|r| r.width).collect()
}

fn compute_case_row_height(case: &Case, columns: &[String], widths: &[u16]) -> u16 {
    let mut max_h = 3u16; // minimum: 1 pad + 1 text + 1 pad
    let num_inner = widths[0].saturating_sub(2);
    let num_text = format!("#{} {}", case.number, case.name);
    max_h = max_h.max(wrap_height(&num_text, num_inner) + 2);
    for (i, col) in columns.iter().enumerate() {
        let value = case.fields.get(col).cloned().unwrap_or_default();
        let inner = widths[i + 1].saturating_sub(2);
        max_h = max_h.max(wrap_height(&value, inner) + 2);
    }
    max_h
}

fn compute_global_row_height(groups: &[Group], columns: &[String], widths: &[u16]) -> u16 {
    groups
        .iter()
        .flat_map(|g| g.cases.iter())
        .map(|case| compute_case_row_height(case, columns, widths))
        .max()
        .unwrap_or(3)
}

fn estimate_para_height(text: &str, width: u16) -> u16 {
    if text.is_empty() || width == 0 {
        return 0;
    }
    wrap_height(text, width)
}

/// Count wrapped lines matching Ratatui's `Paragraph::wrap` (word-boundary
/// breaking). Character-count division underestimates because word boundaries
/// can leave lines shorter than the full width.
fn wrap_height(text: &str, width: u16) -> u16 {
    if width == 0 {
        return 1;
    }
    if text.is_empty() {
        return 1;
    }
    let w = width as usize;
    let mut total = 0u16;
    for line in text.lines() {
        if line.is_empty() {
            total += 1;
            continue;
        }
        total += wrap_line_count(line, w);
    }
    total.max(1)
}

/// Count how many visual lines a single logical line occupies when
/// word-wrapped at `width` columns (greedy line-filling, break long words).
fn wrap_line_count(line: &str, width: usize) -> u16 {
    let mut lines = 1u16;
    let mut col = 0usize;

    for word in line.split(' ') {
        let wlen = word.chars().count();

        if col == 0 {
            // Start of a visual line.
            if wlen <= width {
                col = wlen;
            } else {
                // Forced mid-word break.
                let extra = (wlen - 1) / width;
                lines += extra as u16;
                col = wlen - extra * width;
            }
        } else if col + 1 + wlen <= width {
            // Word + separator space fits on the current line.
            col += 1 + wlen;
        } else {
            // Wrap to the next line.
            lines += 1;
            if wlen <= width {
                col = wlen;
            } else {
                let extra = (wlen - 1) / width;
                lines += extra as u16;
                col = wlen - extra * width;
            }
        }
    }

    lines
}

/// Like `wrap_height` but adds one character for the █ cursor block at the
/// end of the text, which can push to a new wrapped line.
fn wrap_height_with_cursor(text: &str, width: u16) -> u16 {
    if width == 0 {
        return 1;
    }
    let w = width as usize;
    let base = wrap_height(text, width);
    // Check if the last wrapped line is full — if so the cursor wraps.
    let last_line_col = last_col(text, w);
    if last_line_col >= w { base + 1 } else { base }
}

/// Return the column position at the end of the last wrapped line.
fn last_col(text: &str, width: usize) -> usize {
    let mut col = 0usize;
    for line in text.lines() {
        col = 0;
        for word in line.split(' ') {
            let wlen = word.chars().count();
            if col == 0 {
                if wlen <= width {
                    col = wlen;
                } else {
                    col = wlen - ((wlen - 1) / width) * width;
                }
            } else if col + 1 + wlen <= width {
                col += 1 + wlen;
            } else {
                if wlen <= width {
                    col = wlen;
                } else {
                    col = wlen - ((wlen - 1) / width) * width;
                }
            }
        }
    }
    col
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

    // Render Submit before the expansion overlay so a tall expansion can
    // obscure it the same way it obscures other content below the table.
    render_submit_button(buf, plan.submit_y, width, app.is_on_submit());

    // Expanded row overlay: re-render only columns whose content exceeds
    // the capped height. Columns that fit stay at their initial render so
    // they don't blank out rows below in unrelated columns.
    if let Some(ref expanded) = plan.expanded {
        let group = &app.doc.groups[expanded.group_idx];
        let case = &group.cases[expanded.case_idx];
        let layout = &plan.groups[expanded.group_idx];
        let capped = layout.row_height;
        let inner_x = 1u16;

        // Shared height for all non-Decision columns (# + middle columns)
        // so their expansions bottom-align with one continuous separator.
        let num_text = format!("#{} {}", case.number, case.name);
        let num_natural = (wrap_height(&num_text, layout.column_widths[0].saturating_sub(2)) + 2).max(3);
        let shared_h = app.doc.frontmatter.columns.iter().enumerate()
            .filter(|(_, c)| !c.eq_ignore_ascii_case(DECISION_COLUMN))
            .map(|(i, c)| {
                let value = case.fields.get(c).map(String::as_str).unwrap_or("");
                let iw = layout.column_widths[i + 1].saturating_sub(2);
                (wrap_height(value, iw) + 2).max(3)
            })
            .max()
            .unwrap_or(3)
            .max(num_natural);

        // Index of the last non-Decision column (for right-side bar).
        let last_middle = app.doc.frontmatter.columns.iter()
            .rposition(|c| !c.eq_ignore_ascii_case(DECISION_COLUMN));

        // Track the x-range of non-Decision columns for the unified separator.
        let sep_x_start = inner_x;
        let mut sep_x_end = inner_x;

        let mut cx = inner_x;

        // # column
        if shared_h > capped {
            let area = Rect::new(cx, expanded.y, layout.column_widths[0], shared_h);
            let num_inner_w = layout.column_widths[0].saturating_sub(2);
            let text_lines = wrap_height(&num_text, num_inner_w);
            let text_end_col = last_col(&num_text, num_inner_w as usize) as u16;
            prep_expansion_rect(buf, area, text_lines);
            render_text_cell(buf, area, Text::from(num_text), Style::new().fg(COLOR_TEXT_FILLED), false);
            redim_padding(buf, area, text_lines, text_end_col);
            sep_x_end = cx + layout.column_widths[0];
        }
        cx += layout.column_widths[0];

        // Data columns
        for (i, col) in app.doc.frontmatter.columns.iter().enumerate() {
            let value = case.fields.get(col).cloned().unwrap_or_default();
            let is_status = col.eq_ignore_ascii_case(DECISION_COLUMN);
            let col_w = layout.column_widths[i + 1];
            let inner_w = col_w.saturating_sub(2);

            // Decision uses its own height; other columns align to shared_h.
            let col_h = if is_status {
                let h = (wrap_height(&value, inner_w) + 2).max(3);
                h.max(wrap_height_with_cursor(&value, inner_w) + 2)
            } else {
                shared_h
            };

            if col_h > capped {
                let is_selected = is_status
                    && selection
                        .map(|s| s.case == (expanded.group_idx, expanded.case_idx))
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

                let area = Rect::new(cx, expanded.y, col_w, col_h);
                let text_lines = if is_selected {
                    wrap_height_with_cursor(&value, inner_w)
                } else {
                    wrap_height(&value, inner_w)
                };
                let text_end_col = if is_selected && text_lines > wrap_height(&value, inner_w) {
                    // Cursor wrapped to a new line — it's the only char there.
                    1
                } else {
                    last_col(&value, inner_w as usize) as u16
                };
                prep_expansion_rect(buf, area, text_lines);
                render_text_cell(buf, area, cell_text, base_style, false);
                redim_padding(buf, area, text_lines, text_end_col);

                if is_selected {
                    draw_thick_border(
                        buf,
                        area,
                        Style::new()
                            .fg(COLOR_BORDER_SELECTED)
                            .add_modifier(Modifier::BOLD),
                    );
                } else if is_status {
                    draw_thin_border(buf, area, Style::new().fg(Color::Rgb(40, 40, 40)));
                }

                if !is_status {
                    sep_x_end = cx + col_w;
                }
            }

            cx += col_w;
        }

        // Blank the table bottom border where non-Decision expansion obscures
        // it, but preserve cells already overwritten by expansion text. This
        // loop runs after the column render, so resetting indiscriminately
        // would punch a blank line through any text that wrapped down to the
        // table_bottom row.
        let table_bottom = layout.table_y + layout.table_h - 1;
        if shared_h > capped && expanded.y + shared_h > table_bottom {
            for sx in sep_x_start..sep_x_end {
                if let Some(cell) = buf.cell_mut(Position::new(sx, table_bottom))
                    && cell.symbol() == "─"
                {
                    cell.reset();
                }
            }
        }

        // Unified separator line + right bar across all non-Decision columns.
        if shared_h > capped && sep_x_end > sep_x_start {
            let accent = Style::new().fg(COLOR_MARKER);
            let sep_y = expanded.y + shared_h - 1;

            // Horizontal separator '─' across non-Decision columns.
            for sx in sep_x_start..sep_x_end {
                if let Some(cell) = buf.cell_mut(Position::new(sx, sep_y)) {
                    cell.set_char('─').set_style(accent);
                }
            }

            // Connect to the leftmost table border at x=0.
            let connector = Style::new().fg(COLOR_BORDER_FADED);
            let table_bottom = layout.table_y + layout.table_h - 1;
            if sep_y > table_bottom {
                // Expansion extends past the table bottom — extend the left
                // border downward and terminate with └.
                if let Some(cell) = buf.cell_mut(Position::new(0, table_bottom)) {
                    cell.set_char('│').set_style(connector);
                }
                for sy in table_bottom + 1..sep_y {
                    if let Some(cell) = buf.cell_mut(Position::new(0, sy)) {
                        cell.set_char('│').set_style(connector);
                    }
                }
                if let Some(cell) = buf.cell_mut(Position::new(0, sep_y)) {
                    cell.set_char('└').set_style(connector);
                }
            } else if sep_y == table_bottom {
                // Separator coincides with table bottom — L intersection.
                if let Some(cell) = buf.cell_mut(Position::new(0, sep_y)) {
                    cell.set_char('└').set_style(connector);
                }
            } else {
                // Table border continues below — T intersection.
                if let Some(cell) = buf.cell_mut(Position::new(0, sep_y)) {
                    cell.set_char('├').set_style(connector);
                }
            }

            // Right-side bar on the last middle column.
            if let Some(mi) = last_middle {
                let bar_x = inner_x + layout.column_widths[0]
                    + layout.column_widths[1..=mi + 1].iter().sum::<u16>()
                    - 1;
                for sy in expanded.y..expanded.y + shared_h {
                    if let Some(cell) = buf.cell_mut(Position::new(bar_x, sy)) {
                        let ch = if sy == sep_y { '┘' } else { '│' };
                        cell.set_char(ch).set_style(accent);
                    }
                }
            }

        }
    }
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
    let is_focused = selection
        .map(|s| s.case == (group_idx, case_idx))
        .unwrap_or(false);
    let mut cx = base_x;
    let text_h = height.saturating_sub(2);

    // # column
    let num_text = format!("#{} {}", case.number, case.name);
    let num_trunc = wrap_height(&num_text, widths[0].saturating_sub(2)) > text_h;
    let num_style = if is_focused {
        Style::new().fg(COLOR_TEXT_FILLED)
    } else {
        Style::new().fg(Color::Blue)
    };
    render_text_cell(
        buf,
        Rect::new(cx, base_y, widths[0], height),
        Text::from(num_text),
        num_style,
        num_trunc,
    );
    cx += widths[0];

    for (i, col) in columns.iter().enumerate() {
        let value = case.fields.get(col).cloned().unwrap_or_default();
        let is_status = col.eq_ignore_ascii_case(DECISION_COLUMN);
        let is_selected = is_status && is_focused;
        let is_filled = !value.trim().is_empty();

        let base_style = if !is_focused {
            Style::new().fg(Color::Blue)
        } else if is_filled {
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

        let truncated = wrap_height(&value, widths[i + 1].saturating_sub(2)) > text_h;
        let cell_area = Rect::new(cx, base_y, widths[i + 1], height);
        render_text_cell(buf, cell_area, cell_text, base_style, truncated);

        if is_selected {
            draw_thick_border(
                buf,
                cell_area,
                Style::new()
                    .fg(COLOR_BORDER_SELECTED)
                    .add_modifier(Modifier::BOLD),
            );
        } else if is_status {
            draw_thin_border(buf, cell_area, Style::new().fg(Color::Rgb(40, 40, 40)));
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

fn render_text_cell(buf: &mut Buffer, area: Rect, text: Text<'_>, style: Style, truncated: bool) {
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

    if truncated && inner.width > 0 && inner.height > 0 {
        let ex = inner.x + inner.width - 1;
        let ey = inner.y + inner.height - 1;
        if let Some(cell) = buf.cell_mut(Position::new(ex, ey)) {
            cell.set_char('…').set_style(Style::new().fg(COLOR_MARKER));
        }
    }
}

/// Prepare the expansion area: clear the top portion (where expansion text
/// will render) and dim the bottom portion (where obscured row content peeks
/// through). `text_lines` is how many wrapped lines the expansion text uses.
fn prep_expansion_rect(buf: &mut Buffer, area: Rect, text_lines: u16) {
    let inner_top = area.y + 1;
    // Clear everything in the text region + 1 blank separator line.
    let clear_end = (inner_top + text_lines + 1).min(area.y + area.height);
    for y in area.y..clear_end {
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
                cell.reset();
            }
        }
    }
    // Dim the remaining rows (obscured content).
    let dim = Style::new().fg(Color::Rgb(60, 70, 110));
    for y in clear_end..area.y + area.height {
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
                cell.set_style(dim);
            }
        }
    }
}

/// Re-apply dim styling to the inner padding below rendered text.
/// Paragraph::render overwrites the dim fg on the entire inner area;
/// this restores it for lines below the actual text content.
fn redim_padding(buf: &mut Buffer, area: Rect, text_lines: u16, text_end_col: u16) {
    let dim = Style::new().fg(Color::Rgb(60, 70, 110));
    let inner_top = area.y + 1;
    let inner_bottom = area.y + area.height.saturating_sub(1);
    let inner_left = area.x + 1;
    let inner_right = area.x + area.width.saturating_sub(1);

    // Blank the tail of the last text line (after text ends on that line).
    if text_lines > 0 {
        let last_text_y = inner_top + text_lines - 1;
        let tail_start = inner_left + text_end_col;
        for x in tail_start..inner_right {
            if let Some(cell) = buf.cell_mut(Position::new(x, last_text_y)) {
                cell.reset();
            }
        }
    }

    // Blank line immediately after text to visually separate.
    let pad_start = inner_top + text_lines;
    if pad_start < inner_bottom {
        for x in inner_left..inner_right {
            if let Some(cell) = buf.cell_mut(Position::new(x, pad_start)) {
                cell.reset();
            }
        }
    }

    // Remaining padding: dim the obscured content.
    for y in (pad_start + 1)..inner_bottom {
        for x in inner_left..inner_right {
            if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
                cell.set_style(dim);
            }
        }
    }
}

fn draw_thin_border(buf: &mut Buffer, area: Rect, style: Style) {
    if area.width < 2 || area.height < 2 {
        return;
    }
    let r = area.x + area.width - 1;
    let b = area.y + area.height - 1;

    for &(x, y, ch) in &[
        (area.x, area.y, '┌'),
        (r, area.y, '┐'),
        (area.x, b, '└'),
        (r, b, '┘'),
    ] {
        if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
            cell.set_char(ch).set_style(style);
        }
    }

    for x in area.x + 1..r {
        if let Some(cell) = buf.cell_mut(Position::new(x, area.y)) {
            cell.set_char('─').set_style(style);
        }
        if let Some(cell) = buf.cell_mut(Position::new(x, b)) {
            cell.set_char('─').set_style(style);
        }
    }

    for y in area.y + 1..b {
        if let Some(cell) = buf.cell_mut(Position::new(area.x, y)) {
            cell.set_char('│').set_style(style);
        }
        if let Some(cell) = buf.cell_mut(Position::new(r, y)) {
            cell.set_char('│').set_style(style);
        }
    }
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
