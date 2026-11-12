use std::fs;

use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use kido::app::{App, Mode};
use kido::store::Store;
use kido::ui;
use ratatui::backend::TestBackend;
use ratatui::style::{Color, Modifier};
use ratatui::Terminal;
use unicode_width::UnicodeWidthChar;

const SAMPLE: &str = "\
>> 工作
[ ] 写周报
    #note: 周三前发
[x] 回邮件

>> 生活
[ ] 买菜
";

struct Fixture {
    dir: std::path::PathBuf,
    app: App,
    terminal: Terminal<TestBackend>,
}

impl Fixture {
    fn new(name: &str) -> Self {
        Self::with_size(name, 70, 12)
    }

    fn with_size(name: &str, width: u16, height: u16) -> Self {
        let dir = std::env::temp_dir().join(format!("kido-it-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("todo.txt");
        fs::write(&path, SAMPLE).unwrap();
        let app = App::new(Store::load(&path).unwrap());
        let terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut fixture = Self {
            dir,
            app,
            terminal,
        };
        fixture.draw();
        fixture
    }

    fn draw(&mut self) {
        let app = &mut self.app;
        self.terminal.draw(|frame| ui::draw(frame, app)).unwrap();
    }

    fn screen(&self) -> String {
        let buffer = self.terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..buffer.area.height {
            let mut x = 0;
            while x < buffer.area.width {
                let symbol = buffer[(x, y)].symbol();
                out.push_str(symbol);
                // 宽字符后一格是填充，跳过它才能拼出连续的中文
                let width = symbol
                    .chars()
                    .next()
                    .map(|c| UnicodeWidthChar::width(c).unwrap_or(1))
                    .unwrap_or(1);
                x += width.max(1) as u16;
            }
            out.push('\n');
        }
        out
    }

    fn saved(&self) -> String {
        fs::read_to_string(self.dir.join("todo.txt")).unwrap()
    }

    fn key(&mut self, code: KeyCode) {
        self.key_with(code, KeyModifiers::NONE);
    }

    fn key_with(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        self.app
            .on_key(KeyEvent::new_with_kind(code, modifiers, KeyEventKind::Press));
        self.draw();
    }

    fn click(&mut self, button: MouseButton, column: u16, row: u16) {
        self.app.on_mouse(MouseEvent {
            kind: MouseEventKind::Down(button),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        });
        self.draw();
    }

    /// 第一条空白行的屏幕坐标（在最后一个任务下面）。
    fn blank_row(&self) -> u16 {
        self.app.list_area.y + self.app.cur_len() as u16
    }

    fn cell_colors(&self) -> Vec<(Color, u16, u16)> {
        let buffer = self.terminal.backend().buffer();
        let mut out = Vec::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                let cell = &buffer[(x, y)];
                out.push((cell.fg, x, y));
                out.push((cell.bg, x, y));
            }
        }
        out
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn renders_tabs_tasks_and_counter() {
    let fixture = Fixture::new("render");
    let screen = fixture.screen();
    assert!(screen.contains("> 1 工作"), "缺当前页签标记:\n{screen}");
    assert!(screen.contains("2 生活"), "缺第二个页签:\n{screen}");
    assert!(screen.contains("[ ] 写周报"), "缺任务行:\n{screen}");
    assert!(screen.contains("[x] 回邮件"), "缺已完成行:\n{screen}");
    assert!(screen.contains("1/2"), "缺计数:\n{screen}");
}

#[test]
fn note_is_shown_inline_in_secondary_style() {
    let fixture = Fixture::new("inline-note");
    let screen = fixture.screen();
    let line = screen
        .lines()
        .find(|line| line.contains("写周报"))
        .expect("找不到任务行");
    assert!(line.contains("周三前发"), "备注没显示在同一行:\n{screen}");
    assert!(line.contains(" · "), "备注前缺分隔符:\n{screen}");
}

#[test]
fn long_note_is_truncated_with_ellipsis() {
    let mut fixture = Fixture::with_size("truncate", 40, 10);
    fixture.app.store.tabs[0].tasks[0].note =
        "这是一段很长很长的备注，长到一行绝对放不下，应该被截断并显示省略号".to_string();
    fixture.draw();
    let screen = fixture.screen();
    let line = screen.lines().find(|line| line.contains("写周报")).unwrap();
    assert!(line.contains('…'), "超长备注没有省略号:\n{screen}");
    assert!(!line.contains("省略号"), "截断失效:\n{screen}");
}

#[test]
fn status_does_not_hide_key_hints() {
    let mut fixture = Fixture::new("status-row");
    fixture.key(KeyCode::Char(' '));
    let screen = fixture.screen();
    assert!(screen.contains("完成 写周报"), "缺状态提示:\n{screen}");
    assert!(screen.contains("新建"), "按键提示被覆盖:\n{screen}");
    assert!(screen.contains("帮助"), "按键提示被覆盖:\n{screen}");
    let status_y = screen
        .lines()
        .position(|line| line.contains("完成 写周报"))
        .unwrap();
    let hint_y = screen.lines().position(|line| line.contains("帮助")).unwrap();
    assert!(status_y < hint_y, "状态提示应在提示行之上:\n{screen}");
}

#[test]
fn left_click_anywhere_on_row_toggles() {
    let mut fixture = Fixture::new("click");
    let row = fixture.app.list_area.y;
    let x = fixture.app.list_area.x + 20;
    fixture.click(MouseButton::Left, x, row);
    assert!(fixture.app.store.tabs[0].tasks[0].done);
    assert!(fixture.saved().contains("[x] 写周报"));

    fixture.click(MouseButton::Left, fixture.app.list_area.x, row);
    assert!(!fixture.app.store.tabs[0].tasks[0].done);
}

#[test]
fn right_click_opens_note_instead_of_toggling() {
    let mut fixture = Fixture::new("note");
    let row = fixture.app.list_area.y;
    fixture.click(MouseButton::Right, fixture.app.list_area.x + 5, row);
    assert!(!fixture.app.store.tabs[0].tasks[0].done);
    assert!(matches!(fixture.app.mode, Mode::Note(_)));
}

#[test]
fn double_click_on_blank_area_starts_new_task() {
    let mut fixture = Fixture::new("blank-dbl");
    let row = fixture.blank_row();
    let x = fixture.app.list_area.x + 3;
    fixture.click(MouseButton::Left, x, row);
    assert!(matches!(fixture.app.mode, Mode::Normal), "单击不该弹窗");
    fixture.click(MouseButton::Left, x, row);
    assert!(matches!(fixture.app.mode, Mode::Prompt(_)), "双击应弹新建");
}

#[test]
fn right_click_on_blank_area_starts_new_task() {
    let mut fixture = Fixture::new("blank-right");
    let row = fixture.blank_row();
    let x = fixture.app.list_area.x + 3;
    fixture.click(MouseButton::Right, x, row);
    assert!(matches!(fixture.app.mode, Mode::Prompt(_)));
}

#[test]
fn clicking_outside_popup_closes_it() {
    let mut fixture = Fixture::new("outside");
    let row = fixture.app.list_area.y;
    fixture.click(MouseButton::Right, fixture.app.list_area.x + 2, row);
    assert!(matches!(fixture.app.mode, Mode::Note(_)));

    fixture.click(MouseButton::Left, 0, 0);
    assert!(
        matches!(fixture.app.mode, Mode::Normal),
        "点窗口外应关闭弹窗"
    );
}

#[test]
fn clicking_inside_popup_keeps_it_open() {
    let mut fixture = Fixture::new("inside");
    fixture.key(KeyCode::Char('n'));
    let rect = fixture.app.popup_rect;
    assert!(rect.width > 0);
    fixture.click(MouseButton::Left, rect.x + 2, rect.y + 1);
    assert!(matches!(fixture.app.mode, Mode::Prompt(_)));
}

#[test]
fn tasks_can_be_moved_and_sent_to_top() {
    let mut fixture = Fixture::new("reorder");
    assert_eq!(fixture.app.store.tabs[0].tasks[0].text, "写周报");

    fixture.key(KeyCode::Char('J'));
    assert_eq!(fixture.app.store.tabs[0].tasks[0].text, "回邮件");
    assert_eq!(fixture.app.sel, 1);

    fixture.key(KeyCode::Char('K'));
    assert_eq!(fixture.app.store.tabs[0].tasks[0].text, "写周报");

    fixture.key(KeyCode::End);
    assert_eq!(fixture.app.sel, 1);
    fixture.key_with(KeyCode::Home, KeyModifiers::CONTROL);
    assert_eq!(fixture.app.store.tabs[0].tasks[0].text, "回邮件");
    assert_eq!(fixture.app.sel, 0);
}

#[test]
fn alt_arrows_reorder_tasks() {
    let mut fixture = Fixture::new("reorder-alt");
    fixture.key(KeyCode::End);
    fixture.key_with(KeyCode::Up, KeyModifiers::ALT);
    assert_eq!(fixture.app.sel, 0);
    assert_eq!(fixture.app.store.tabs[0].tasks[0].text, "回邮件");
    assert_eq!(fixture.app.store.tabs[0].tasks[1].text, "写周报");
}

#[test]
fn clicking_a_tab_switches() {
    let mut fixture = Fixture::new("tabswitch");
    let (x0, x1, idx) = fixture.app.tab_ranges[1];
    assert_eq!(idx, 1);
    let x = x0 + (x1 - x0) / 2;
    let row = fixture.app.tabs_row;
    fixture.click(MouseButton::Left, x, row);
    assert_eq!(fixture.app.tab, 1);
    assert_eq!(fixture.app.store.tabs[1].tasks[0].text, "买菜");
}

#[test]
fn right_click_on_tab_asks_to_delete_it() {
    let mut fixture = Fixture::new("tabdel");
    let (x0, x1, _) = fixture.app.tab_ranges[1];
    let x = x0 + (x1 - x0) / 2;
    let row = fixture.app.tabs_row;
    fixture.click(MouseButton::Right, x, row);
    assert!(matches!(fixture.app.mode, Mode::Confirm(_)));
    fixture.key(KeyCode::Char('y'));
    assert_eq!(fixture.app.store.tabs.len(), 1);
    assert!(!fixture.saved().contains("买菜"));
}

#[test]
fn new_task_prompt_writes_to_disk() {
    let mut fixture = Fixture::new("add");
    fixture.key(KeyCode::Char('n'));
    for ch in "整理周报".chars() {
        fixture.key(KeyCode::Char(ch));
    }
    fixture.key(KeyCode::Enter);
    assert_eq!(fixture.app.cur_len(), 3);
    assert!(fixture.saved().contains("[ ] 整理周报"));
}

#[test]
fn space_toggles_and_u_undoes() {
    let mut fixture = Fixture::new("undo");
    fixture.key(KeyCode::Char(' '));
    assert!(fixture.app.store.tabs[0].tasks[0].done);
    fixture.key(KeyCode::Char('u'));
    assert!(!fixture.app.store.tabs[0].tasks[0].done);
    assert!(fixture.saved().contains("[ ] 写周报"));
}

#[test]
fn note_editor_round_trips_multiline() {
    let mut fixture = Fixture::new("noteedit");
    fixture.key(KeyCode::Char('e'));
    fixture.key(KeyCode::Enter);
    for ch in "第二行".chars() {
        fixture.key(KeyCode::Char(ch));
    }
    fixture.key(KeyCode::Esc);
    assert_eq!(fixture.app.store.tabs[0].tasks[0].note, "周三前发\n第二行");
    assert!(fixture.saved().contains("#note: 第二行"));
}

#[test]
fn tabs_can_be_added_renamed_and_deleted() {
    let mut fixture = Fixture::new("tabcrud");
    fixture.key(KeyCode::Char('t'));
    for ch in "读书".chars() {
        fixture.key(KeyCode::Char(ch));
    }
    fixture.key(KeyCode::Enter);
    assert_eq!(fixture.app.store.tabs.len(), 3);
    assert_eq!(fixture.app.tab, 2);
    assert!(fixture.saved().contains(">> 读书"));

    fixture.key(KeyCode::Char('T'));
    fixture.key(KeyCode::Backspace);
    fixture.key(KeyCode::Char('籍'));
    fixture.key(KeyCode::Enter);
    assert_eq!(fixture.app.store.tabs[2].name, "读籍");

    fixture.key(KeyCode::Char('D'));
    assert!(matches!(fixture.app.mode, Mode::Confirm(_)));
    fixture.key(KeyCode::Char('y'));
    assert_eq!(fixture.app.store.tabs.len(), 2);
    assert!(!fixture.saved().contains("读籍"));
}

#[test]
fn delete_task_can_be_undone() {
    let mut fixture = Fixture::new("deltask");
    fixture.key(KeyCode::Char('d'));
    assert_eq!(fixture.app.cur_len(), 1);
    fixture.key(KeyCode::Char('u'));
    assert_eq!(fixture.app.cur_len(), 2);
    assert!(fixture.saved().contains("[ ] 写周报"));
}

#[test]
fn external_edit_is_picked_up() {
    let mut fixture = Fixture::new("reload");
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(fixture.dir.join("todo.txt"), ">> 外部\n[ ] 换过了\n").unwrap();
    fixture.app.tick();
    fixture.draw();
    assert_eq!(fixture.app.store.tabs[0].name, "外部");
    assert!(fixture.screen().contains("换过了"));
}

/// 只用终端 16 色，才能被 Windows Terminal 的配色方案染色。
#[test]
fn only_ansi_palette_colors_are_used() {
    let mut fixture = Fixture::new("palette");
    fixture.key(KeyCode::Char(' '));
    let mut used: Vec<Color> = Vec::new();
    for (color, x, y) in fixture.cell_colors() {
        assert!(
            !matches!(color, Color::Rgb(..)),
            "({x},{y}) 用了 truecolor，会绕开终端配色方案"
        );
        used.push(color);
    }
    assert!(
        used.contains(&Color::Green) || used.contains(&Color::Blue),
        "页签强调色没有落在 ANSI 调色板上"
    );
}

#[test]
fn selection_uses_reverse_video_instead_of_a_hardcoded_background() {
    let fixture = Fixture::new("reverse");
    let row = fixture.app.list_area.y;
    let buffer = fixture.terminal.backend().buffer();
    let cell = &buffer[(2, row)];
    assert!(
        cell.modifier.contains(Modifier::REVERSED),
        "选中行应该用反显，而不是写死底色"
    );
    assert!(!matches!(cell.bg, Color::Rgb(..)));
}

