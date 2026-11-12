//! 渲染：页签栏 / 任务列表 / 状态行 / 快捷键提示 / 弹窗。

use std::path::Path;

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::app::{App, ConfirmState, Mode, NoteState, PromptState, StatusKind};
use crate::store::Task;
use crate::theme;

const HINTS: &[(&str, &str)] = &[
    ("n", "新建"),
    ("Space", "勾选"),
    ("e", "备注"),
    ("d", "删除"),
    ("Tab", "切换页签"),
    ("?", "帮助"),
];

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    if area.width < 24 || area.height < 8 {
        frame.render_widget(
            Paragraph::new("窗口太小").style(Style::default().fg(theme::warn())),
            area,
        );
        return;
    }

    let [tabs_area, rule_top, list_area, rule_bottom, status_area, hint_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_tabs(frame, app, tabs_area);
    draw_rule(frame, rule_top);
    draw_list(frame, app, list_area);
    draw_rule(frame, rule_bottom);
    draw_status(frame, app, status_area);
    draw_hint(frame, hint_area);
    draw_mode(frame, app, area);
}

fn draw_rule(frame: &mut Frame, area: Rect) {
    let line = Line::from(Span::styled(
        "─".repeat(area.width as usize),
        Style::default().fg(theme::dim()),
    ));
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_tabs(frame: &mut Frame, app: &mut App, area: Rect) {
    let counter = {
        let tab = app.cur();
        (
            tab.map_or(0, |t| t.done_count()),
            tab.map_or(0, |t| t.tasks.len()),
        )
    };
    let counter_text = format!("{}/{}", counter.0, counter.1);
    let counter_width = UnicodeWidthStr::width(counter_text.as_str()) as u16;

    let mut tabs_area = area;
    let mut counter_area = area;
    if area.width > counter_width + 8 {
        tabs_area.width = area.width - counter_width - 3;
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
        let color = theme::accent(idx);
        let label = format!("{}{} {}", if active { "> " } else { "  " }, idx + 1, tab.name);
        let width = UnicodeWidthStr::width(label.as_str()) as u16;
        if x + width > tabs_area.right() {
            break;
        }
        ranges.push((x, x + width, idx));
        let style = if active {
            Style::default()
                .fg(color)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(color)
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::styled("│", Style::default().fg(theme::dim())));
        x += width + 1;
    }
    app.tab_ranges = ranges;
    app.tabs_row = area.y;

    frame.render_widget(Paragraph::new(Line::from(spans)), tabs_area);
    if counter_area.width > 0 {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                counter_text,
                Style::default().fg(theme::dim()),
            )))
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
    let accent = theme::accent(app.tab);

    let mut lines: Vec<Line> = Vec::new();
    if total == 0 {
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                "空 · 双击空白处或按 n 新建任务",
                Style::default().fg(theme::dim()),
            ),
        ]));
    } else {
        let end = (app.scroll + height).min(total);
        for idx in app.scroll..end {
            let task = &app.store.tabs[app.tab].tasks[idx];
            let selected = idx == app.sel;
            lines.push(Line::from(task_spans(task, width, selected, accent)));
        }
    }

    frame.render_widget(Paragraph::new(lines), area);
}

/// 选中行用反显做高亮块（方案无关的高对比），页签强调色只用在左侧竖条与复选框上。
/// 备注永远是不反显的灰字，所以高亮块在任务名结束处收尾。
fn task_spans(task: &Task, width: usize, selected: bool, accent: Color) -> Vec<Span<'static>> {
    let base = if selected {
        theme::selection()
    } else {
        Style::default()
    };
    let mut spans: Vec<Span> = Vec::new();

    spans.push(Span::styled(
        if selected { "▌ " } else { "  " },
        if selected {
            base.fg(accent)
        } else {
            Style::default()
        },
    ));

    let mark_style = if selected {
        base.fg(accent)
    } else if task.done {
        Style::default().fg(theme::dim())
    } else {
        Style::default().fg(accent)
    };
    spans.push(Span::styled(
        if task.done { "[x] " } else { "[ ] " },
        mark_style,
    ));

    let indent = 6;
    let avail = width.saturating_sub(indent);
    let text_style = if selected {
        base.fg(theme::text()).add_modifier(Modifier::BOLD)
    } else if task.done {
        theme::done()
    } else {
        Style::default().fg(theme::text())
    };
    // 选中行要做成三段色块：复选框=页签强调色底、任务名=默认前景色底、
    // 备注=灰底。灰底靠"反显 + DIM"实现——终端会把反转后的底色压暗，
    // 这样不用写死任何 RGB，换方案时三段色块会一起变。
    let note_style = if selected {
        base.add_modifier(Modifier::DIM)
    } else {
        Style::default().fg(theme::dim())
    };

    let note = first_line(&task.note);
    let mut used = indent;

    match note {
        None => {
            let text = truncate(&task.text, avail);
            used += UnicodeWidthStr::width(text.as_str());
            spans.push(Span::styled(text, text_style));
        }
        Some(note) => {
            // 任务名最多占 2/3，留出位置显示备注摘要
            let text_cap = (avail * 2 / 3).max(6);
            let text = truncate(&task.text, text_cap);
            let text_width = UnicodeWidthStr::width(text.as_str());
            used += text_width;
            spans.push(Span::styled(text, text_style));

            let rest = avail.saturating_sub(text_width);
            if rest > 4 {
                let note_text = truncate(note, rest - 3);
                let note_width = UnicodeWidthStr::width(note_text.as_str());
                used += 3 + note_width;
                spans.push(Span::styled(" · ", note_style));
                spans.push(Span::styled(note_text, note_style));
            }
        }
    }

    if used < width {
        // 反显只覆盖文字部分，形成"高亮块"而不是整行色带
        spans.push(Span::raw(" ".repeat(width - used)));
    }
    spans
}

fn first_line(note: &str) -> Option<&str> {
    note.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let span = match &app.status {
        Some(status) => {
            let color = match status.kind {
                StatusKind::Info => theme::info(),
                StatusKind::Ok => theme::ok(),
                StatusKind::Warn => theme::warn(),
                StatusKind::Error => theme::error(),
            };
            Span::styled(format!(" {}", status.message), Style::default().fg(color))
        }
        None => Span::raw(""),
    };
    frame.render_widget(Paragraph::new(Line::from(span)), area);
}

fn draw_hint(frame: &mut Frame, area: Rect) {
    let mut spans: Vec<Span> = vec![Span::raw(" ")];
    for (i, (key, desc)) in HINTS.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", Style::default().fg(theme::dim())));
        }
        spans.push(Span::styled(*key, Style::default().fg(theme::info())));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(*desc, Style::default().fg(theme::dim())));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_mode(frame: &mut Frame, app: &mut App, area: Rect) {
    let mode = std::mem::replace(&mut app.mode, Mode::Normal);
    let popup = match &mode {
        Mode::Normal => Rect::default(),
        Mode::Prompt(state) => draw_prompt(frame, state, area),
        Mode::Note(state) => draw_note(frame, state, area),
        Mode::Confirm(state) => draw_confirm(frame, state, area),
        Mode::Help => draw_help(frame, &app.store.path, area),
    };
    app.mode = mode;
    app.popup_rect = popup;
}

fn draw_prompt(frame: &mut Frame, state: &PromptState, area: Rect) -> Rect {
    let width = 56.min(area.width.saturating_sub(4)).max(16);
    let rect = centered(area, width, 3);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::info()))
        .title(Span::styled(
            format!(" {} ", state.title()),
            Style::default()
                .fg(theme::info())
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            state.input.value(),
            Style::default().fg(theme::text()),
        ))),
        inner,
    );

    let before = state.input.before_cursor();
    let offset = (UnicodeWidthStr::width(before.as_str()) as u16).min(inner.width.saturating_sub(1));
    frame.set_cursor_position((inner.x + offset, inner.y));
    rect
}

fn draw_note(frame: &mut Frame, state: &NoteState, area: Rect) -> Rect {
    let width = area.width.saturating_sub(8).clamp(30, 84);
    let height = area.height.saturating_sub(8).clamp(8, 24);
    let rect = centered(area, width, height);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::info()))
        .title(Span::styled(
            " 备注 ",
            Style::default()
                .fg(theme::info())
                .add_modifier(Modifier::BOLD),
        ));
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
        lines.push(Line::from(Span::styled(
            skip_cells(chars, hscroll),
            Style::default().fg(theme::text()),
        )));
    }
    frame.render_widget(Paragraph::new(lines), text_area);

    let hint = format!(
        " Enter 换行 · Esc / Ctrl+S 保存 · 点窗口外也能关 · {} 行",
        row_count
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            hint,
            Style::default().fg(theme::dim()),
        ))),
        hint_area,
    );

    let cursor_cells = cells(&state.editor.lines[state.editor.row][..state.editor.col]);
    let hscroll = if cursor_cells >= text_area.width as usize {
        cursor_cells + 1 - text_area.width as usize
    } else {
        0
    };
    let offset = (cursor_cells - hscroll).min(text_area.width.saturating_sub(1) as usize) as u16;
    let cursor_row = state.editor.row.saturating_sub(scroll) as u16;
    frame.set_cursor_position((text_area.x + offset, text_area.y + cursor_row));
    rect
}

fn draw_confirm(frame: &mut Frame, state: &ConfirmState, area: Rect) -> Rect {
    let width = 56.min(area.width.saturating_sub(4)).max(20);
    let rect = centered(area, width, 5);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::warn()))
        .title(Span::styled(
            " 确认 ",
            Style::default()
                .fg(theme::warn())
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let text = Text::from(vec![
        Line::from(Span::styled(
            state.question.clone(),
            Style::default().fg(theme::text()),
        )),
        Line::from(Span::styled(
            " y 确认 · 其他键或点窗口外取消 · u 事后可撤销",
            Style::default().fg(theme::dim()),
        )),
    ]);
    frame.render_widget(Paragraph::new(text), inner);
    rect
}

fn draw_help(frame: &mut Frame, path: &Path, area: Rect) -> Rect {
    let pairs: &[(&str, &str)] = &[
        ("j / ↓", "下移"),
        ("k / ↑", "上移"),
        ("Space", "勾选 / 取消"),
        ("e / Enter", "编辑备注"),
        ("n", "新建任务"),
        ("r", "重命名任务"),
        ("d", "删除任务"),
        ("K / Alt+↑", "任务上移一位"),
        ("J / Alt+↓", "任务下移一位"),
        ("Ctrl+Home", "任务移到首位"),
        ("Ctrl+End", "任务移到末尾"),
        ("u / Ctrl+Z", "撤销"),
        ("t / T / D", "新建 / 改名 / 删除页签"),
        ("Tab / h / l", "切换页签"),
        ("o", "用默认程序打开数据文件"),
        ("q / Esc", "退出"),
    ];

    let width = 62.min(area.width.saturating_sub(4)).max(30);
    let height = (pairs.len() + 7) as u16;
    let rect = centered(area, width, height);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::info()))
        .title(Span::styled(
            " 帮助 · 任意键关闭 ",
            Style::default()
                .fg(theme::info())
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let mut lines: Vec<Line> = pairs
        .iter()
        .map(|(key, desc)| {
            Line::from(vec![
                Span::styled(format!("  {key:<12}"), Style::default().fg(theme::info())),
                Span::styled(desc.to_string(), Style::default().fg(theme::text())),
            ])
        })
        .collect();
    lines.push(Line::from(Span::styled(
        "  鼠标：左键 = 勾选 · 右键 = 备注",
        Style::default().fg(theme::dim()),
    )));
    lines.push(Line::from(Span::styled(
        "        空白处双击 / 右键 = 新建 · 右键页签 = 删除",
        Style::default().fg(theme::dim()),
    )));
    lines.push(Line::from(Span::styled(
        format!("  数据：{}", path.display()),
        Style::default().fg(theme::dim()),
    )));
    lines.push(Line::from(Span::styled(
        "  配色：只用 ANSI 16 色，跟随终端配色方案",
        Style::default().fg(theme::dim()),
    )));

    frame.render_widget(Paragraph::new(lines), inner);
    rect
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
