//! 文本编辑缓冲：单行输入（新建/改名）与多行备注编辑器。
//!
//! 光标位置用「字符下标」而不是字节下标，中文、emoji 都不会把字符切坏。

#[derive(Clone, Debug, Default)]
pub struct LineInput {
    chars: Vec<char>,
    cursor: usize,
}

impl LineInput {
    pub fn new(text: &str) -> Self {
        let chars: Vec<char> = text.chars().collect();
        let cursor = chars.len();
        Self { chars, cursor }
    }

    pub fn value(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    pub fn before_cursor(&self) -> String {
        self.chars[..self.cursor].iter().collect()
    }

    pub fn insert(&mut self, c: char) {
        self.chars.insert(self.cursor, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.chars.len() {
            self.chars.remove(self.cursor);
        }
    }

    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn right(&mut self) {
        if self.cursor < self.chars.len() {
            self.cursor += 1;
        }
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.chars.len();
    }

    /// 删除光标前的一个单词（Ctrl+Backspace）。
    pub fn backspace_word(&mut self) {
        while self.cursor > 0 && self.chars[self.cursor - 1].is_whitespace() {
            self.backspace();
        }
        while self.cursor > 0 && !self.chars[self.cursor - 1].is_whitespace() {
            self.backspace();
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct NoteEditor {
    pub lines: Vec<Vec<char>>,
    pub row: usize,
    pub col: usize,
}

impl NoteEditor {
    pub fn from_text(text: &str) -> Self {
        let mut lines: Vec<Vec<char>> = text.lines().map(|line| line.chars().collect()).collect();
        if lines.is_empty() {
            lines.push(Vec::new());
        }
        let row = lines.len() - 1;
        let col = lines[row].len();
        Self { lines, row, col }
    }

    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(|line| line.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn line(&self) -> &Vec<char> {
        &self.lines[self.row]
    }

    pub fn insert(&mut self, c: char) {
        self.lines[self.row].insert(self.col, c);
        self.col += 1;
    }

    pub fn newline(&mut self) {
        let tail = self.lines[self.row].split_off(self.col);
        self.row += 1;
        self.lines.insert(self.row, tail);
        self.col = 0;
    }

    pub fn backspace(&mut self) {
        if self.col > 0 {
            self.col -= 1;
            self.lines[self.row].remove(self.col);
        } else if self.row > 0 {
            let current = self.lines.remove(self.row);
            self.row -= 1;
            self.col = self.line().len();
            self.lines[self.row].extend(current);
        }
    }

    pub fn delete(&mut self) {
        if self.col < self.line().len() {
            self.lines[self.row].remove(self.col);
        } else if self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].extend(next);
        }
    }

    pub fn left(&mut self) {
        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.line().len();
        }
    }

    pub fn right(&mut self) {
        if self.col < self.line().len() {
            self.col += 1;
        } else if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        }
    }

    pub fn up(&mut self) {
        if self.row > 0 {
            self.row -= 1;
            self.col = self.col.min(self.line().len());
        }
    }

    pub fn down(&mut self) {
        if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = self.col.min(self.line().len());
        }
    }

    pub fn home(&mut self) {
        self.col = 0;
    }

    pub fn end(&mut self) {
        self.col = self.line().len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_input_handles_cjk_by_char() {
        let mut input = LineInput::new("写周报");
        assert_eq!(input.value(), "写周报");
        assert_eq!(input.before_cursor(), "写周报");
        input.backspace();
        assert_eq!(input.value(), "写周");
        input.home();
        input.insert('A');
        assert_eq!(input.value(), "A写周");
        assert_eq!(input.before_cursor(), "A");
    }

    #[test]
    fn note_editor_joins_and_splits_lines() {
        let mut editor = NoteEditor::from_text("第一行\n第二行");
        assert_eq!(editor.row, 1);
        assert_eq!(editor.col, 3);
        editor.home();
        editor.newline();
        assert_eq!(editor.text(), "第一行\n\n第二行");
        editor.backspace();
        assert_eq!(editor.text(), "第一行\n第二行");
    }

    #[test]
    fn note_editor_merges_lines_on_backspace() {
        let mut editor = NoteEditor::from_text("ab\ncd");
        editor.home();
        editor.backspace();
        assert_eq!(editor.text(), "abcd");
        assert_eq!(editor.col, 2);
    }

    #[test]
    fn empty_note_starts_with_one_line() {
        let editor = NoteEditor::from_text("");
        assert_eq!(editor.lines.len(), 1);
        assert_eq!(editor.text(), "");
    }
}
