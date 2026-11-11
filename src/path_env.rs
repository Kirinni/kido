//! 用户 PATH 的读写（HKCU\Environment）。
//!
//! 用注册表原始值读写，避免 `setx` 的 1024 字符截断，也避免把 REG_EXPAND_SZ
//! 里的 `%USERPROFILE%` 之类展开后写死。

use std::io;
use std::path::Path;

use winreg::enums::*;
use winreg::{RegKey, RegValue};

fn open() -> io::Result<RegKey> {
    RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE)
}

fn decode(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let end = units.iter().position(|&unit| unit == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end])
}

fn encode(text: &str) -> Vec<u8> {
    let mut bytes: Vec<u8> = text.encode_utf16().flat_map(|unit| unit.to_le_bytes()).collect();
    bytes.extend_from_slice(&[0, 0]);
    bytes
}

fn read_path(key: &RegKey) -> (String, RegType) {
    match key.get_raw_value("Path") {
        Ok(value) => (decode(&value.bytes), value.vtype),
        Err(_) => (String::new(), RegType::REG_EXPAND_SZ),
    }
}

fn same(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.trim().trim_end_matches('\\').to_ascii_lowercase();
    norm(a) == norm(b)
}

/// 把目录加入用户 PATH，返回是否真的改了。
pub fn add_dir(dir: &Path) -> io::Result<bool> {
    let key = open()?;
    let target = dir.to_string_lossy().to_string();
    let (current, vtype) = read_path(&key);
    if current
        .split(';')
        .any(|entry| !entry.trim().is_empty() && same(entry, &target))
    {
        return Ok(false);
    }
    let mut updated = current.trim_end_matches(';').to_string();
    if !updated.is_empty() {
        updated.push(';');
    }
    updated.push_str(&target);
    key.set_raw_value(
        "Path",
        &RegValue {
            bytes: encode(&updated).into(),
            vtype,
        },
    )?;
    Ok(true)
}

/// 从用户 PATH 里移除目录，返回是否真的改了。
pub fn remove_dir(dir: &Path) -> io::Result<bool> {
    let key = open()?;
    let target = dir.to_string_lossy().to_string();
    let (current, vtype) = read_path(&key);
    let entries: Vec<&str> = current.split(';').filter(|e| !e.trim().is_empty()).collect();
    let kept: Vec<&str> = entries
        .iter()
        .copied()
        .filter(|entry| !same(entry, &target))
        .collect();
    if kept.len() == entries.len() {
        return Ok(false);
    }
    key.set_raw_value(
        "Path",
        &RegValue {
            bytes: encode(&kept.join(";")).into(),
            vtype,
        },
    )?;
    Ok(true)
}
