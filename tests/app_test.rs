use std::fs;

use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use kido::app::{App, Mode};
use kido::store::Store;
use kido::ui;
use ratatui::backend::TestBackend;
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
        let dir = std::env::temp_dir().join(format!("kido-it-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("todo.txt");
        fs::write(&path, SAMPLE).unwrap();
        let app = App::new(Store::load(&path).unwrap());
        let terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
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
    assert!(screen.contains("> 工作"), "缺当前页签标记:\n{screen}");
    assert!(screen.contains("生活"), "缺第二个页签:\n{screen}");
    assert!(screen.contains("[ ] 写周报"), "缺任务行:\n{screen}");
    assert!(screen.contains("[x] 回邮件"), "缺已完成行:\n{screen}");
    assert!(screen.contains("1/2"), "缺计数:\n{screen}");
    assert!(screen.contains('✎'), "缺备注标记:\n{screen}");
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

