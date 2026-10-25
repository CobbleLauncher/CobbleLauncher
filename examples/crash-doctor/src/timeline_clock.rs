// SPDX-License-Identifier: MIT
//! Time-of-day evidence only. No date, elapsed process time, or resource sampling.
pub(super) fn clock(line: &str) -> Option<(u64, &str)> {
    let value = line.strip_prefix('[')?.split_once(']')?.0;
    if !value.is_ascii() || value.len() < 8 || &value[2..3] != ":" || &value[5..6] != ":" {
        return None;
    }
    let h: u64 = value[..2].parse().ok()?;
    let m: u64 = value[3..5].parse().ok()?;
    let s: u64 = value[6..8].parse().ok()?;
    if h > 23
        || m > 59
        || s > 59
        || !value[..2].bytes().all(|b| b.is_ascii_digit())
        || !value[3..5].bytes().all(|b| b.is_ascii_digit())
        || !value[6..8].bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let fraction = if value.len() == 8 {
        0
    } else {
        if !matches!(value.as_bytes()[8], b'.' | b',') {
            return None;
        }
        let tail = &value[9..];
        if tail.is_empty() || tail.len() > 3 || !tail.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        tail.parse::<u64>().ok()? * 10u64.pow(3 - tail.len() as u32)
    };
    Some(((h * 3600 + m * 60 + s) * 1000 + fraction, value))
}
/// Accept standard [thread/LEVEL]: and Fabric's [LEVEL] [context/category]:.
pub(super) fn message(line: &str) -> Option<(&str, bool)> {
    let (header, message) = line.split_once("]: ")?;
    let level = header.split('[').find_map(|group| {
        let group = group.trim_end_matches([']', ' ']);
        let level = group.rsplit('/').next()?;
        ["INFO", "WARN", "ERROR", "DEBUG", "TRACE", "FATAL"]
            .contains(&level)
            .then_some(level)
    })?;
    Some((message, level == "WARN"))
}
