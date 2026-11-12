//! 配色：只用终端的 16 色 ANSI 调色板，不用 truecolor。
//!
//! Windows Terminal 的「配色方案」覆盖的正是这 16 个槽位，所以换方案时 kido 会跟着
//! 重新着色；一旦写死 RGB，就会绕过方案、永远是同一套颜色。
//!
//! 角色划分：
//! - 正文用 `Reset`（方案里的默认前景色），不猜颜色；
//! - 次要信息（备注、提示、分隔线）用 bright black（槽位 8），这是多数方案里
//!   专门留给"灰字"的槽位；
//! - 六个页签强调色依次取 green/blue/magenta/cyan/yellow/bright cyan，避开红色
//!   以便把红留给错误；
//! - 选中行不涂底色，改用反显（REVERSED），这样在任何方案下都有对比度。

use ratatui::style::{Color, Modifier, Style};

/// 页签强调色，按序号循环。
pub const ACCENTS: [Color; 6] = [
    Color::Green,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::Yellow,
    Color::LightCyan,
];

pub fn accent(tab: usize) -> Color {
    ACCENTS[tab % ACCENTS.len()]
}

/// 正文：跟随方案的默认前景色。
pub fn text() -> Color {
    Color::Reset
}

/// 次要文字：备注、提示、分隔符。
pub fn dim() -> Color {
    Color::DarkGray
}

/// 已完成任务。
pub fn done() -> Style {
    Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::CROSSED_OUT)
}

/// 选中行：反显，不依赖具体底色，任何方案下都看得清。
pub fn selection() -> Style {
    Style::default().add_modifier(Modifier::REVERSED)
}

pub fn info() -> Color {
    Color::Cyan
}

pub fn ok() -> Color {
    Color::Green
}

pub fn warn() -> Color {
    Color::Yellow
}

pub fn error() -> Color {
    Color::Red
}
