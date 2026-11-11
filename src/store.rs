//! 数据层：`todo.txt` 的解析、序列化与原子写入。
//!
//! 文件格式（与 Rainmeter 皮肤兼容）：
//!
//! ```text
//! >> 工作                 <- 页签
//! [ ] 写周报              <- 未完成
//! [x] 回邮件              <- 已完成
//!     #note: 周三前发     <- 上一条任务的备注（缩进，可多行）
//!
//! >> 生活
//! - 买菜                  <- `- ` `* ` `+ ` 也认，视为未完成
//! # 整行注释，忽略
//! ```
//!
//! 备注保存时会去掉每行首尾空白；空行用 `    #note: ` 保留。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const NOTE_PREFIX: &str = "#note:";
pub const DEFAULT_TAB: &str = "todo";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Task {
    pub text: String,
    pub done: bool,
    pub note: String,
}

impl Task {
    pub fn has_note(&self) -> bool {
        !self.note.trim().is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tab {
    pub name: String,
    pub tasks: Vec<Task>,
}

impl Tab {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tasks: Vec::new(),
        }
    }

    pub fn done_count(&self) -> usize {
        self.tasks.iter().filter(|t| t.done).count()
    }
}

pub struct Store {
    pub tabs: Vec<Tab>,
    pub path: PathBuf,
    stamp: Option<SystemTime>,
}

impl Store {
    pub fn load(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
            Err(err) => return Err(err),
        };
        let stamp = mtime(&path);
        Ok(Self {
            tabs: parse(&text),
            path,
            stamp,
        })
    }

    /// 先写临时文件再替换，避免中途崩溃留下半截文件。
    pub fn save(&mut self) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            if !dir.as_os_str().is_empty() {
                fs::create_dir_all(dir)?;
            }
        }
        let text = serialize(&self.tabs);
        let tmp = self.path.with_extension("txt.tmp");
        fs::write(&tmp, text.as_bytes())?;
        fs::rename(&tmp, &self.path)?;
        self.stamp = mtime(&self.path);
        Ok(())
    }

    /// 外部（编辑器 / Rainmeter 皮肤）改过文件时重新载入，返回是否发生了重载。
    pub fn reload_if_changed(&mut self) -> io::Result<bool> {
        let stamp = mtime(&self.path);
        if stamp == self.stamp {
            return Ok(false);
        }
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
            Err(err) => return Err(err),
        };
        self.tabs = parse(&text);
        self.stamp = stamp;
        Ok(true)
    }
}

fn mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).ok().and_then(|meta| meta.modified().ok())
}

/// 数据文件位置：`KIDO_HOME` > exe 同目录 > `%LOCALAPPDATA%\kido`。
///
/// 装到 `%LOCALAPPDATA%\kido\kido.exe` 时两者一致，整台机器只多出这一个目录。
pub fn default_data_path() -> PathBuf {
    if let Some(home) = std::env::var_os("KIDO_HOME") {
        if !home.is_empty() {
            return PathBuf::from(home).join("todo.txt");
        }
    }
    if let Some(dir) = exe_dir() {
        return dir.join("todo.txt");
    }
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("kido").join("todo.txt")
}

pub fn exe_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.parent().map(Path::to_path_buf)
}

/// 默认安装目录，`--uninstall` 用它判断哪些东西是自己装的。
pub fn install_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|base| base.join("kido"))
}

pub fn parse(text: &str) -> Vec<Tab> {
    let mut tabs: Vec<Tab> = Vec::new();
    let mut current: Option<usize> = None;

    for raw in text.lines() {
        let line = raw.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        let line = line.trim_start();

        if let Some(rest) = line.strip_prefix(">>") {
            tabs.push(Tab::new(rest.trim()));
            current = Some(tabs.len() - 1);
            continue;
        }

        if let Some(rest) = line.strip_prefix(NOTE_PREFIX) {
            let content = rest.strip_prefix(' ').unwrap_or(rest);
            if let Some(tab) = current.and_then(|i| tabs.get_mut(i)) {
                if let Some(task) = tab.tasks.last_mut() {
                    if !task.note.is_empty() {
                        task.note.push('\n');
                    }
                    task.note.push_str(content);
                }
            }
            continue;
        }

        if line.starts_with('#') {
            continue;
        }

        if let Some((done, rest)) = split_checkbox(line) {
            push_task(&mut tabs, &mut current, rest, done);
            continue;
        }

        let bullet = line
            .strip_prefix("- ")
            .or_else(|| line.strip_prefix("* "))
            .or_else(|| line.strip_prefix("+ "));
        if let Some(rest) = bullet {
            push_task(&mut tabs, &mut current, rest, false);
        }
    }

    if tabs.is_empty() {
        tabs.push(Tab::new(DEFAULT_TAB));
    }
    tabs
}

fn split_checkbox(line: &str) -> Option<(bool, &str)> {
    let mut chars = line.chars();
    if chars.next()? != '[' {
        return None;
    }
    let mark = chars.next()?;
    if chars.next()? != ']' {
        return None;
    }
    let done = match mark {
        'x' | 'X' => true,
        ' ' | '-' | '_' => false,
        _ => return None,
    };
    let rest = &line[line.char_indices().nth(3)?.0..];
    Some((done, rest.trim_start()))
}

fn push_task(tabs: &mut Vec<Tab>, current: &mut Option<usize>, text: &str, done: bool) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    if current.is_none() {
        tabs.push(Tab::new(DEFAULT_TAB));
        *current = Some(0);
    }
    if let Some(tab) = current.and_then(|i| tabs.get_mut(i)) {
        tab.tasks.push(Task {
            text: text.to_string(),
            done,
            note: String::new(),
        });
    }
}

pub fn serialize(tabs: &[Tab]) -> String {
    let mut out = String::new();
    for (i, tab) in tabs.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(">> ");
        out.push_str(tab.name.trim());
        out.push('\n');
        for task in &tab.tasks {
            out.push('[');
            out.push(if task.done { 'x' } else { ' ' });
            out.push_str("] ");
            out.push_str(task.text.trim());
            out.push('\n');
            for line in normalized_note(&task.note) {
                out.push_str("    ");
                out.push_str(NOTE_PREFIX);
                out.push(' ');
                out.push_str(&line);
                out.push('\n');
            }
        }
    }
    out
}

/// 备注规范化：逐行去首尾空白、去掉首尾空行。保存和比较都用它。
pub fn normalize_note(note: &str) -> String {
    normalized_note(note).join("\n")
}

fn normalized_note(note: &str) -> Vec<String> {
    let mut lines: Vec<String> = note.lines().map(|line| line.trim().to_string()).collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    while lines.first().is_some_and(|line| line.is_empty()) {
        lines.remove(0);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
>> 工作
[ ] 写周报
    #note: 周三前发
    #note: 附上数据
[x] 回邮件

>> 生活
[ ] 买菜
";

    #[test]
    fn parses_tabs_tasks_and_notes() {
        let tabs = parse(SAMPLE);
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].name, "工作");
        assert_eq!(tabs[0].tasks.len(), 2);
        assert_eq!(tabs[0].tasks[0].text, "写周报");
        assert!(!tabs[0].tasks[0].done);
        assert_eq!(tabs[0].tasks[0].note, "周三前发\n附上数据");
        assert!(tabs[0].tasks[1].done);
        assert_eq!(tabs[1].tasks[0].text, "买菜");
        assert_eq!(tabs[0].done_count(), 1);
    }

    #[test]
    fn round_trips() {
        let tabs = parse(SAMPLE);
        assert_eq!(serialize(&tabs), SAMPLE);
    }

    #[test]
    fn accepts_bullets_comments_and_crlf() {
        let text = "# 注释\r\n>> 页签\r\n- 第一条\r\n* 第二条\r\n+ 第三条\r\n任意杂行\r\n";
        let tabs = parse(text);
        assert_eq!(tabs.len(), 1);
        let texts: Vec<&str> = tabs[0].tasks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["第一条", "第二条", "第三条"]);
    }

    #[test]
    fn empty_file_gets_a_default_tab() {
        let tabs = parse("");
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0].name, DEFAULT_TAB);
        assert!(tabs[0].tasks.is_empty());
    }

    #[test]
    fn tasks_before_any_tab_land_in_default_tab() {
        let tabs = parse("[ ] 散装任务\n");
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0].name, DEFAULT_TAB);
        assert_eq!(tabs[0].tasks[0].text, "散装任务");
    }

    #[test]
    fn note_before_any_task_is_ignored() {
        let tabs = parse(">> 工作\n    #note: 孤立备注\n[ ] 任务\n");
        assert_eq!(tabs[0].tasks.len(), 1);
        assert_eq!(tabs[0].tasks[0].note, "");
    }

    #[test]
    fn multiline_notes_survive_blank_lines() {
        let tabs = vec![Tab {
            name: "t".into(),
            tasks: vec![Task {
                text: "a".into(),
                done: false,
                note: "第一行\n\n第三行".into(),
            }],
        }];
        let text = serialize(&tabs);
        assert_eq!(parse(&text)[0].tasks[0].note, "第一行\n\n第三行");
    }

    #[test]
    fn checkbox_variants() {
        let tabs = parse("[X] 大写完成\n[-] 横线未完成\n[ ] 空格未完成\n");
        assert!(tabs[0].tasks[0].done);
        assert!(!tabs[0].tasks[1].done);
        assert!(!tabs[0].tasks[2].done);
    }

    #[test]
    fn task_text_keeps_leading_bracket() {
        let tabs = parse(">> todo\n[ ] [draft] 标题\n");
        assert_eq!(tabs[0].tasks[0].text, "[draft] 标题");
        assert_eq!(serialize(&tabs), ">> todo\n[ ] [draft] 标题\n");
    }

    #[test]
    fn save_then_reload_sees_external_change() {
        let dir = std::env::temp_dir().join(format!("kido-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("todo.txt");

        let mut store = Store::load(&path).unwrap();
        store.tabs[0].tasks.push(Task {
            text: "本地".into(),
            ..Default::default()
        });
        store.save().unwrap();
        assert!(!store.reload_if_changed().unwrap());

        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(&path, ">> 外部\n[ ] 改过了\n").unwrap();
        assert!(store.reload_if_changed().unwrap());
        assert_eq!(store.tabs[0].name, "外部");

        let _ = fs::remove_dir_all(&dir);
    }
}
