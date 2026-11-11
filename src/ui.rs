//! 渲染：页签栏 / 任务列表 / 状态栏 / 弹窗。

use std::path::Path;

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::app::{App, ConfirmState, Mode, NoteState, PromptState};

const ACCENT: Color = Color::Rgb(92, 232, 168);
const DIM: Color = Color::Rgb(108, 126, 132);
const TEXT: Color = Color::Rgb(226, 236, 239);
const SEL_BG: Color = Color::Rgb(31, 56, 48);
const WARN: Color = Color::Rgb(240, 180, 90);

const HINT: &str =
    " n 新建 · Space 勾选 · e 备注 · d 删除 · t 页签 · Tab 切换 · ? 帮助";

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    if area.width < 24 || area.height < 6 {
        frame.render_widget(
            Paragraph::new("窗口太小").style(Style::default().fg(WARN)),
            area,
        );
        return;
    }

    let [tabs_area, rule_top, list_area, rule_bottom, status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_tabs(frame, app, tabs_area);
    draw_rule(frame, rule_top);
    draw_list(frame, app, list_area);
    draw_rule(frame, rule_bottom);
    draw_status(frame, app, status_area);
    draw_mode(frame, app, area);
}

fn draw_rule(frame: &mut Frame, area: Rect) {
    let line = Line::from(Span::styled(
        "─".repeat(area.width as usize),
        Style::default().fg(DIM),
    ));
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_tabs(frame: &mut Frame, app: &mut App, area: Rect) {
    let counter = {
        let tab = app.cur();
        format!(
            "{}/{}",
            tab.map_or(0, |t| t.done_count()),
            tab.map_or(0, |t| t.tasks.len())
        )
    };
    let counter_width = UnicodeWidthStr::width(counter.as_str()) as u16;

    let mut tabs_area = area;
    let mut counter_area = area;
    if area.width > counter_width + 6 {
        tabs_area.width = area.width - counter_width - 2;
        counter_area.x = area.right() - counter_width;
        counter_area.width = counter_width;
    } else {
        counter_area.width = 0;
    }

    let mut spans: Vec<Span> = Vec::new();
    let mut ranges: Vec<(u16, u16, usize)> = Vec::new();
    let mut x = tabs_area.x;
    for (idx, tab) in app.store.tabs.iter().enumerate() {
        let active = idx == app.tab;
        let label = format!("{}{}", if active { "> " } else { "  " }, tab.name);
        let width = UnicodeWidthStr::width(label.as_str()) as u16;
        if x + width > tabs_area.right() {
            break;
        }
        ranges.push((x, x + width, idx));
        let style = if active {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(DIM)
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw("  "));
        x += width + 2;
    }
    app.tab_ranges = ranges;
    app.tabs_row = area.y;

    frame.render_widget(Paragraph::new(Line::from(spans)), tabs_area);
    if counter_area.width > 0 {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(counter, Style::default().fg(DIM))))
                .alignment(Alignment::Right),
            counter_area,
        );
    }
}

fn draw_list(frame: &mut Frame, app: &mut App, area: Rect) {
    app.list_area = area;
    let height = area.height as usize;
    app.ensure_visible(height);
    let width = area.width as usize;
    let total = app.cur_len();

    let mut lines: Vec<Line> = Vec::new();
    if total == 0 {
        lines.push(Line::from(Span::styled(
            "  空 · 按 n 新建任务",
            Style::default().fg(DIM),
        )));
    } else {
        let end = (app.scroll + height).min(total);
        for idx in app.scroll..end {
            let task = &app.store.tabs[app.tab].tasks[idx];
            let selected = idx == app.sel;
            let has_note = task.has_note();
            let room = width.saturating_sub(6 + usize::from(has_note) * 2);
            let text = truncate(&task.text, room);
            let used = 6 + UnicodeWidthStr::width(text.as_str()) + usize::from(has_note) * 2;

            let text_style = if task.done {
                Style::default().fg(DIM).add_modifier(Modifier::CROSSED_OUT)
            } else {
                Style::default().fg(TEXT)
            };
            let mark_style = if task.done {
                Style::default().fg(DIM)
            } else {
                Style::default().fg(ACCENT)
            };
            let gutter_style = if selected {
                Style::default().fg(ACCENT)
            } else {
                Style::default()
            };

            let mut spans = vec![
                Span::styled(if selected { "▌ " } else { "  " }, gutter_style),
                Span::styled(if task.done { "[x] " } else { "[ ] " }, mark_style),
                Span::styled(text, text_style),
            ];
            if has_note {
                spans.push(Span::styled(" ✎", Style::default().fg(DIM)));
            }
            if used < width {
                spans.push(Span::raw(" ".repeat(width - used)));
            }

            let mut line = Line::from(spans);
            if selected {
                line = line.style(Style::default().bg(SEL_BG));
            }
            lines.push(line);
        }
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let span = match &app.status {
        Some((message, _)) => Span::styled(format!(" {message}"), Style::default().fg(ACCENT)),
        None => Span::styled(HINT, Style::default().fg(DIM)),
    };
    frame.render_widget(Paragraph::new(Line::from(span)), area);
}

fn draw_mode(frame: &mut Frame, app: &App, area: Rect) {
    match &app.mode {
        Mode::Normal => {}
        Mode::Prompt(state) => draw_prompt(frame, state, area),
        Mode::Note(state) => draw_note(frame, state, area),
        Mode::Confirm(state) => draw_confirm(frame, state, area),
        Mode::Help => draw_help(frame, &app.store.path, area),
    }
}

fn draw_prompt(frame: &mut Frame, state: &PromptState, area: Rect) {
    let width = 56.min(area.width.saturating_sub(4)).max(16);
    let rect = centered(area, width, 3);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(
            format!(" {} ", state.title()),
            Style::default().fg(ACCENT),
        ));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let text = state.input.value();
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(text, Style::default().fg(TEXT)))),
        inner,
    );

    let before = state.input.before_cursor();
    let offset = (UnicodeWidthStr::width(before.as_str()) as u16).min(inner.width.saturating_sub(1));
    frame.set_cursor_position((inner.x + offset, inner.y));
}

fn draw_note(frame: &mut Frame, state: &NoteState, area: Rect) {
    let width = area.width.saturating_sub(8).clamp(30, 84);
    let height = area.height.saturating_sub(6).clamp(8, 26);
    let rect = centered(area, width, height);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(" 备注 ", Style::default().fg(ACCENT)));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let [text_area, hint_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
    let view_height = text_area.height.max(1) as usize;
    let row_count = state.editor.lines.len();
    let scroll = if state.editor.row >= view_height {
        state.editor.row + 1 - view_height
    } else {
        0
    };

    let mut lines: Vec<Line> = Vec::new();
    for row in scroll..(scroll + view_height).min(row_count) {
        let chars = &state.editor.lines[row];
        let cursor_cells = if row == state.editor.row {
            cells(&chars[..state.editor.col])
        } else {
            0
        };
        let hscroll = if row == state.editor.row && cursor_cells >= text_area.width as usize {
            cursor_cells + 1 - text_area.width as usize
        } else {
            0
        };
        let visible = skip_cells(chars, hscroll);
        lines.push(Line::from(Span::styled(visible, Style::default().fg(TEXT))));
    }
    frame.render_widget(Paragraph::new(lines), text_area);

    let hint = format!(
        " Enter 换行 · Ctrl+S / Esc 保存 · u 可撤销 · {} 行",
        row_count
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(hint, Style::default().fg(DIM)))),
        hint_area,
    );

    let cursor_cells = cells(&state.editor.lines[state.editor.row][..state.editor.col]);
    let offset = if state.editor.row >= scroll {
        let hscroll = if cursor_cells >= text_area.width as usize {
            cursor_cells + 1 - text_area.width as usize
        } else {
            0
        };
        (cursor_cells - hscroll).min(text_area.width.saturating_sub(1) as usize) as u16
    } else {
        0
    };
    let cursor_row = state.editor.row.saturating_sub(scroll) as u16;
    frame.set_cursor_position((text_area.x + offset, text_area.y + cursor_row));
}

fn draw_confirm(frame: &mut Frame, state: &ConfirmState, area: Rect) {
    let width = 54.min(area.width.saturating_sub(4)).max(20);
    let rect = centered(area, width, 5);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(WARN))
        .title(Span::styled(" 确认 ", Style::default().fg(WARN)));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let text = Text::from(vec![
        Line::from(Span::styled(state.question.clone(), Style::default().fg(TEXT))),
        Line::from(Span::styled(
            " y 确认 · 其他键取消 · u 事后可撤销",
            Style::default().fg(DIM),
        )),
    ]);
    frame.render_widget(Paragraph::new(text), inner);
}

fn draw_help(frame: &mut Frame, path: &Path, area: Rect) {
    let pairs: &[(&str, &str)] = &[
        ("j / ↓", "下移"),
        ("k / ↑", "上移"),
        ("Space", "勾选 / 取消"),
        ("e / Enter", "编辑备注"),
        ("n", "新建任务"),
        ("r", "重命名任务"),
        ("d", "删除任务"),
        ("u / Ctrl+Z", "撤销"),
        ("t / T / D", "新建 / 改名 / 删除页签"),
        ("Tab / h / l", "切换页签"),
        ("o", "用默认程序打开数据文件"),
        ("q / Esc", "退出"),
    ];

    let width = 60.min(area.width.saturating_sub(4)).max(30);
    let height = (pairs.len() + 5) as u16;
    let rect = centered(area, width, height);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(" 帮助 · 任意键关闭 ", Style::default().fg(ACCENT)));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let mut lines: Vec<Line> = pairs
        .iter()
        .map(|(key, desc)| {
            Line::from(vec![
                Span::styled(format!("  {key:<12}"), Style::default().fg(ACCENT)),
                Span::styled(desc.to_string(), Style::default().fg(TEXT)),
            ])
        })
        .collect();
    lines.push(Line::from(Span::styled(
        "  鼠标：左键 = 勾选 · 右键 = 备注".to_string(),
        Style::default().fg(DIM),
    )));
    lines.push(Line::from(Span::styled(
        "        点页签 = 切换 · 滚轮 = 移动".to_string(),
        Style::default().fg(DIM),
    )));
    lines.push(Line::from(Span::styled(
        format!("  数据：{}", path.display()),
        Style::default().fg(DIM),
    )));

    frame.render_widget(Paragraph::new(lines), inner);
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

fn cells(chars: &[char]) -> usize {
    chars
        .iter()
        .map(|c| UnicodeWidthChar::width(*c).unwrap_or(0))
        .sum()
}

/// 跳过前 `offset` 个显示单元格，返回剩余文本（用于横向滚动）。
fn skip_cells(chars: &[char], offset: usize) -> String {
    let mut skipped = 0;
    let mut out = String::new();
    for &c in chars {
        if skipped < offset {
            skipped += UnicodeWidthChar::width(c).unwrap_or(0);
            continue;
        }
        out.push(c);
    }
    out
}

fn truncate(text: &str, max_cells: usize) -> String {
    if UnicodeWidthStr::width(text) <= max_cells {
        return text.to_string();
    }
    if max_cells == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut width = 0;
    for c in text.chars() {
        let cw = UnicodeWidthChar::width(c).unwrap_or(0);
        if width + cw > max_cells.saturating_sub(1) {
            break;
        }
        out.push(c);
        width += cw;
    }
    out.push('…');
    out
}
