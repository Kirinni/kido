//! 交互状态机：页签 / 选中行 / 撤销栈 / 弹窗模式。

use std::time::{Duration, Instant};

use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;

use crate::editor::{LineInput, NoteEditor};
use crate::store::{Store, Tab, Task, DEFAULT_TAB};

const STATUS_TTL: Duration = Duration::from_secs(3);
const UNDO_LIMIT: usize = 200;

pub enum Mode {
    Normal,
    Prompt(PromptState),
    Note(NoteState),
    Confirm(ConfirmState),
    Help,
}

pub enum PromptKind {
    NewTask,
    RenameTask(usize),
    NewTab,
    RenameTab(usize),
}

pub struct PromptState {
    pub kind: PromptKind,
    pub input: LineInput,
}

impl PromptState {
    pub fn title(&self) -> &'static str {
        match self.kind {
            PromptKind::NewTask => "新建任务",
            PromptKind::RenameTask(_) => "重命名任务",
            PromptKind::NewTab => "新建页签",
            PromptKind::RenameTab(_) => "重命名页签",
        }
    }
}

pub struct NoteState {
    pub tab: usize,
    pub idx: usize,
    pub editor: NoteEditor,
}

pub enum ConfirmAction {
    DeleteTab(usize),
}

pub struct ConfirmState {
    pub question: String,
    pub action: ConfirmAction,
}

pub struct App {
    pub store: Store,
    pub tab: usize,
    pub sel: usize,
    pub scroll: usize,
    pub mode: Mode,
    pub status: Option<(String, Instant)>,
    pub quit: bool,
    /// 上一次绘制得到的列表区域，鼠标命中测试用。
    pub list_area: Rect,
    /// 页签所在行与每个页签的横向范围。
    pub tabs_row: u16,
    pub tab_ranges: Vec<(u16, u16, usize)>,
    undo: Vec<Vec<Tab>>,
}

impl App {
    pub fn new(mut store: Store) -> Self {
        if store.tabs.is_empty() {
            store.tabs.push(Tab::new(DEFAULT_TAB));
        }
        Self {
            store,
            tab: 0,
            sel: 0,
            scroll: 0,
            mode: Mode::Normal,
            status: None,
            quit: false,
            list_area: Rect::default(),
            tabs_row: 0,
            tab_ranges: Vec::new(),
            undo: Vec::new(),
        }
    }

    pub fn cur(&self) -> Option<&Tab> {
        self.store.tabs.get(self.tab)
    }

    pub fn cur_len(&self) -> usize {
        self.cur().map_or(0, |tab| tab.tasks.len())
    }

    pub fn cur_task(&self) -> Option<&Task> {
        self.cur().and_then(|tab| tab.tasks.get(self.sel))
    }

    pub fn list_height(&self) -> usize {
        self.list_area.height.max(1) as usize
    }

    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status = Some((message.into(), Instant::now()));
    }

    pub fn tick(&mut self) {
        let expired = self
            .status
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() >= STATUS_TTL);
        if expired {
            self.status = None;
        }
        if matches!(self.mode, Mode::Normal) {
            match self.store.reload_if_changed() {
                Ok(true) => {
                    self.clamp();
                    self.set_status("已从磁盘重新载入");
                }
                Ok(false) => {}
                Err(err) => self.set_status(format!("读取失败: {err}")),
            }
        }
    }

    fn clamp(&mut self) {
        if self.store.tabs.is_empty() {
            self.store.tabs.push(Tab::new(DEFAULT_TAB));
        }
        if self.tab >= self.store.tabs.len() {
            self.tab = self.store.tabs.len() - 1;
        }
        let len = self.cur_len();
        if len == 0 {
            self.sel = 0;
        } else if self.sel >= len {
            self.sel = len - 1;
        }
        let height = self.list_height();
        self.ensure_visible(height);
    }

    pub fn ensure_visible(&mut self, height: usize) {
        let height = height.max(1);
        if self.sel < self.scroll {
            self.scroll = self.sel;
        }
        if self.sel >= self.scroll + height {
            self.scroll = self.sel + 1 - height;
        }
        let max_scroll = self.cur_len().saturating_sub(height);
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }
    }

    fn snapshot(&mut self) {
        self.undo.push(self.store.tabs.clone());
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
    }

    fn save(&mut self) {
        if let Err(err) = self.store.save() {
            self.set_status(format!("保存失败: {err}"));
        }
    }

    // ---------------------------------------------------------------- 编辑操作

    pub fn toggle(&mut self, idx: usize) {
        if idx >= self.cur_len() {
            return;
        }
        self.snapshot();
        let message = {
            let Some(task) = self
                .store
                .tabs
                .get_mut(self.tab)
                .and_then(|tab| tab.tasks.get_mut(idx))
            else {
                return;
            };
            task.done = !task.done;
            format!("{} {}", if task.done { "完成" } else { "取消" }, task.text)
        };
        self.save();
        self.set_status(message);
    }

    pub fn add_task(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        self.snapshot();
        if let Some(tab) = self.store.tabs.get_mut(self.tab) {
            tab.tasks.push(Task {
                text: text.to_string(),
                done: false,
                note: String::new(),
            });
        }
        self.sel = self.cur_len().saturating_sub(1);
        let height = self.list_height();
        self.ensure_visible(height);
        self.save();
        self.set_status(format!("已添加 {text}"));
    }

    pub fn rename_task(&mut self, idx: usize, text: &str) {
        let text = text.trim();
        if text.is_empty() || idx >= self.cur_len() {
            return;
        }
        self.snapshot();
        if let Some(task) = self
            .store
            .tabs
            .get_mut(self.tab)
            .and_then(|tab| tab.tasks.get_mut(idx))
        {
            task.text = text.to_string();
        }
        self.save();
        self.set_status(format!("已改名 {text}"));
    }

    pub fn delete_task(&mut self, idx: usize) {
        if idx >= self.cur_len() {
            return;
        }
        self.snapshot();
        let removed = self
            .store
            .tabs
            .get_mut(self.tab)
            .and_then(|tab| (idx < tab.tasks.len()).then(|| tab.tasks.remove(idx).text));
        self.clamp();
        self.save();
        if let Some(text) = removed {
            self.set_status(format!("已删除 {text} · u 撤销"));
        }
    }

    pub fn set_note(&mut self, tab_idx: usize, idx: usize, note: &str) {
        let normalized = crate::store::normalize_note(note);
        let changed = self
            .store
            .tabs
            .get(tab_idx)
            .and_then(|tab| tab.tasks.get(idx))
            .is_some_and(|task| task.note != normalized);
        if !changed {
            return;
        }
        self.snapshot();
        if let Some(task) = self
            .store
            .tabs
            .get_mut(tab_idx)
            .and_then(|tab| tab.tasks.get_mut(idx))
        {
            task.note = normalized;
        }
        self.save();
        self.set_status("备注已保存 · u 撤销");
    }

    pub fn add_tab(&mut self, name: &str) {
        let name = name.trim();
        let name = if name.is_empty() { "新页签" } else { name };
        self.snapshot();
        self.store.tabs.push(Tab::new(name));
        self.tab = self.store.tabs.len() - 1;
        self.sel = 0;
        self.scroll = 0;
        self.save();
        self.set_status(format!("已新建页签 {name}"));
    }

    pub fn rename_tab(&mut self, idx: usize, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        self.snapshot();
        if let Some(tab) = self.store.tabs.get_mut(idx) {
            tab.name = name.to_string();
        }
        self.save();
        self.set_status(format!("页签已改名 {name}"));
    }

    pub fn delete_tab(&mut self, idx: usize) {
        if self.store.tabs.len() <= 1 {
            self.set_status("至少保留一个页签");
            return;
        }
        if idx >= self.store.tabs.len() {
            return;
        }
        self.snapshot();
        let removed = self.store.tabs.remove(idx);
        if self.tab > idx {
            self.tab -= 1;
        }
        if self.tab >= self.store.tabs.len() {
            self.tab = self.store.tabs.len() - 1;
        }
        self.sel = 0;
        self.scroll = 0;
        self.save();
        self.set_status(format!("已删除页签 {} · u 撤销", removed.name));
    }

    pub fn undo(&mut self) {
        match self.undo.pop() {
            Some(tabs) => {
                self.store.tabs = tabs;
                self.clamp();
                self.save();
                self.set_status("已撤销");
            }
            None => self.set_status("没有可撤销的操作"),
        }
    }

    // ---------------------------------------------------------------- 导航

    pub fn move_sel(&mut self, delta: i32) {
        let len = self.cur_len();
        if len == 0 {
            return;
        }
        let next = self.sel as i64 + delta as i64;
        self.sel = next.clamp(0, len as i64 - 1) as usize;
        let height = self.list_height();
        self.ensure_visible(height);
    }

    pub fn switch_tab(&mut self, idx: usize) {
        if idx >= self.store.tabs.len() || idx == self.tab {
            return;
        }
        self.tab = idx;
        self.sel = 0;
        self.scroll = 0;
    }

    pub fn cycle_tab(&mut self, delta: i32) {
        let count = self.store.tabs.len();
        if count == 0 {
            return;
        }
        let next = (self.tab as i32 + delta).rem_euclid(count as i32) as usize;
        self.switch_tab(next);
    }

    // ---------------------------------------------------------------- 输入事件

    pub fn on_key(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        let mode = std::mem::replace(&mut self.mode, Mode::Normal);
        match mode {
            // key_normal 内部可能自己切进弹窗，所以这里不能覆盖 self.mode
            Mode::Normal => self.key_normal(key),
            Mode::Prompt(state) => self.mode = self.key_prompt(key, state),
            Mode::Note(state) => self.mode = self.key_note(key, state),
            Mode::Confirm(state) => self.mode = self.key_confirm(key, state),
            Mode::Help => self.mode = Mode::Normal,
        }
    }

    fn key_normal(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('c') if ctrl => self.quit = true,
            KeyCode::Esc | KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('j') | KeyCode::Down => self.move_sel(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_sel(-1),
            KeyCode::Char('g') | KeyCode::Home => {
                self.sel = 0;
                self.scroll = 0;
            }
            KeyCode::Char('G') | KeyCode::End => {
                self.sel = self.cur_len().saturating_sub(1);
                let height = self.list_height();
                self.ensure_visible(height);
            }
            KeyCode::Tab | KeyCode::Char('l') | KeyCode::Right => self.cycle_tab(1),
            KeyCode::BackTab | KeyCode::Char('h') | KeyCode::Left => self.cycle_tab(-1),
            KeyCode::Char(' ') => {
                let idx = self.sel;
                self.toggle(idx);
            }
            KeyCode::Enter | KeyCode::Char('e') => self.start_note(),
            KeyCode::Char('n') | KeyCode::Char('a') => self.start_prompt(PromptKind::NewTask),
            KeyCode::Char('r') => self.start_prompt(PromptKind::RenameTask(self.sel)),
            KeyCode::Char('d') | KeyCode::Delete => {
                let idx = self.sel;
                self.delete_task(idx);
            }
            KeyCode::Char('u') => self.undo(),
            KeyCode::Char('z') if ctrl => self.undo(),
            KeyCode::Char('t') => self.start_prompt(PromptKind::NewTab),
            KeyCode::Char('T') => self.start_prompt(PromptKind::RenameTab(self.tab)),
            KeyCode::Char('D') => {
                let idx = self.tab;
                self.ask_delete_tab(idx);
            }
            KeyCode::Char('o') => self.open_data_file(),
            KeyCode::Char('?') => self.mode = Mode::Help,
            _ => {}
        }
    }

    fn key_prompt(&mut self, key: KeyEvent, mut state: PromptState) -> Mode {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Enter => {
                let text = state.input.value();
                match state.kind {
                    PromptKind::NewTask => self.add_task(&text),
                    PromptKind::RenameTask(idx) => self.rename_task(idx, &text),
                    PromptKind::NewTab => self.add_tab(&text),
                    PromptKind::RenameTab(idx) => self.rename_tab(idx, &text),
                }
                Mode::Normal
            }
            KeyCode::Esc => Mode::Normal,
            KeyCode::Backspace if ctrl => {
                state.input.backspace_word();
                Mode::Prompt(state)
            }
            KeyCode::Backspace => {
                state.input.backspace();
                Mode::Prompt(state)
            }
            KeyCode::Delete => {
                state.input.delete();
                Mode::Prompt(state)
            }
            KeyCode::Left => {
                state.input.left();
                Mode::Prompt(state)
            }
            KeyCode::Right => {
                state.input.right();
                Mode::Prompt(state)
            }
            KeyCode::Home => {
                state.input.home();
                Mode::Prompt(state)
            }
            KeyCode::End => {
                state.input.end();
                Mode::Prompt(state)
            }
            KeyCode::Char(c) if !ctrl => {
                state.input.insert(c);
                Mode::Prompt(state)
            }
            _ => Mode::Prompt(state),
        }
    }

    fn key_note(&mut self, key: KeyEvent, mut state: NoteState) -> Mode {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => {
                self.set_note(state.tab, state.idx, &state.editor.text());
                Mode::Normal
            }
            KeyCode::Char('s') if ctrl => {
                self.set_note(state.tab, state.idx, &state.editor.text());
                Mode::Normal
            }
            KeyCode::Enter => {
                state.editor.newline();
                Mode::Note(state)
            }
            KeyCode::Tab => {
                state.editor.insert(' ');
                state.editor.insert(' ');
                Mode::Note(state)
            }
            KeyCode::Backspace => {
                state.editor.backspace();
                Mode::Note(state)
            }
            KeyCode::Delete => {
                state.editor.delete();
                Mode::Note(state)
            }
            KeyCode::Left => {
                state.editor.left();
                Mode::Note(state)
            }
            KeyCode::Right => {
                state.editor.right();
                Mode::Note(state)
            }
            KeyCode::Up => {
                state.editor.up();
                Mode::Note(state)
            }
            KeyCode::Down => {
                state.editor.down();
                Mode::Note(state)
            }
            KeyCode::Home => {
                state.editor.home();
                Mode::Note(state)
            }
            KeyCode::End => {
                state.editor.end();
                Mode::Note(state)
            }
            KeyCode::Char(c) if !ctrl => {
                state.editor.insert(c);
                Mode::Note(state)
            }
            _ => Mode::Note(state),
        }
    }

    fn key_confirm(&mut self, key: KeyEvent, state: ConfirmState) -> Mode {
        if matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter) {
            match state.action {
                ConfirmAction::DeleteTab(idx) => self.delete_tab(idx),
            }
        }
        Mode::Normal
    }

    pub fn on_mouse(&mut self, event: MouseEvent) {
        match event.kind {
            MouseEventKind::ScrollUp => self.move_sel(-1),
            MouseEventKind::ScrollDown => self.move_sel(1),
            MouseEventKind::Down(button) => self.on_click(button, event),
            _ => {}
        }
    }

    fn on_click(&mut self, button: MouseButton, event: MouseEvent) {
        if !matches!(self.mode, Mode::Normal) {
            return;
        }
        if event.row == self.tabs_row {
            let hit = self
                .tab_ranges
                .iter()
                .find(|(x0, x1, _)| event.column >= *x0 && event.column < *x1)
                .map(|(_, _, idx)| *idx);
            if let Some(idx) = hit {
                self.switch_tab(idx);
            }
            return;
        }

        let area = self.list_area;
        if area.width == 0 || area.height == 0 {
            return;
        }
        if !area.contains((event.column, event.row).into()) {
            return;
        }
        let idx = self.scroll + (event.row - area.y) as usize;
        if idx >= self.cur_len() {
            return;
        }
        self.sel = idx;
        let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
        match button {
            MouseButton::Left if ctrl => self.start_note(),
            MouseButton::Left => self.toggle(idx),
            MouseButton::Right => self.start_note(),
            _ => {}
        }
    }

    // ---------------------------------------------------------------- 弹窗

    fn start_prompt(&mut self, kind: PromptKind) {
        let initial = match &kind {
            PromptKind::RenameTask(idx) => self
                .store
                .tabs
                .get(self.tab)
                .and_then(|tab| tab.tasks.get(*idx))
                .map(|task| task.text.clone())
                .unwrap_or_default(),
            PromptKind::RenameTab(idx) => self
                .store
                .tabs
                .get(*idx)
                .map(|tab| tab.name.clone())
                .unwrap_or_default(),
            _ => String::new(),
        };
        self.mode = Mode::Prompt(PromptState {
            kind,
            input: LineInput::new(&initial),
        });
    }

    fn start_note(&mut self) {
        let Some(task) = self.cur_task() else {
            return;
        };
        let note = task.note.clone();
        self.mode = Mode::Note(NoteState {
            tab: self.tab,
            idx: self.sel,
            editor: NoteEditor::from_text(&note),
        });
    }

    fn ask_delete_tab(&mut self, idx: usize) {
        if self.store.tabs.len() <= 1 {
            self.set_status("至少保留一个页签");
            return;
        }
        let Some(tab) = self.store.tabs.get(idx) else {
            return;
        };
        self.mode = Mode::Confirm(ConfirmState {
            question: format!("删除页签「{}」及其 {} 个任务？", tab.name, tab.tasks.len()),
            action: ConfirmAction::DeleteTab(idx),
        });
    }

    fn open_data_file(&mut self) {
        let path = self.store.path.clone();
        self.save();
        let spawned = open_in_default_app(&path);
        match spawned {
            Ok(()) => self.set_status(format!("已打开 {}", path.display())),
            Err(err) => self.set_status(format!("打不开 {}: {err}", path.display())),
        }
    }
}

fn open_in_default_app(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .arg("/C")
            .arg("start")
            .arg("")
            .arg(path)
            .spawn()
            .map(|_| ())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map(|_| ())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map(|_| ())
    }
}
