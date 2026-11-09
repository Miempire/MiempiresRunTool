//! 客户端日志：本地加密队列 + 批量上传服务器。
//!
//! 服务器按账号、按天保存；每天凌晨 4 点清理前一天的日志。
//! 上传失败时留在本地队列，下次登录/联网后补传。

use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogEntry {
    /// 本地时间（ISO8601，含时区）。
    pub t: String,
    pub level: String,
    pub msg: String,
}

const MAX_ENTRIES: usize = 600;
const MAX_MSG_CHARS: usize = 800;

fn queue() -> &'static Mutex<Vec<LogEntry>> {
    static QUEUE: OnceLock<Mutex<Vec<LogEntry>>> = OnceLock::new();
    QUEUE.get_or_init(|| {
        let loaded: Vec<LogEntry> =
            crate::secure_store::read_json(crate::secure_store::LOG_QUEUE).unwrap_or_default();
        Mutex::new(loaded)
    })
}

fn clean_message(msg: &str) -> Option<String> {
    let text = msg.trim();
    if text.is_empty() {
        return None;
    }
    let mut out: String = text.chars().take(MAX_MSG_CHARS).collect();
    if text.chars().count() > MAX_MSG_CHARS {
        out.push('…');
    }
    // 避免把可能存在的凭据写进日志队列。
    if out.contains("Bearer ") || out.contains("mrt_") {
        out = out
            .split_whitespace()
            .map(|part| {
                if part.starts_with("mrt_") || part.starts_with("Bearer") {
                    "***"
                } else {
                    part
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
    }
    Some(out)
}

pub fn push(level: &str, msg: &str) {
    let Some(msg) = clean_message(msg) else {
        return;
    };
    let entry = LogEntry {
        t: chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
        level: level.to_string(),
        msg,
    };
    let snapshot = {
        let mut guard = queue().lock().unwrap();
        guard.push(entry);
        if guard.len() > MAX_ENTRIES {
            let drop = guard.len() - MAX_ENTRIES;
            guard.drain(..drop);
        }
        if guard.len() % 5 == 0 {
            Some(guard.clone())
        } else {
            None
        }
    };
    if let Some(snapshot) = snapshot {
        let _ = crate::secure_store::write_json(crate::secure_store::LOG_QUEUE, &snapshot);
    }
}

pub fn info(msg: &str) {
    push("info", msg);
}

pub fn warn(msg: &str) {
    push("warn", msg);
}

pub fn error(msg: &str) {
    push("error", msg);
}

/// 取待上传批次（JSON 结构，t 为字符串）。
pub fn pending_json(limit: usize) -> Vec<serde_json::Value> {
    let guard = queue().lock().unwrap();
    guard
        .iter()
        .take(limit)
        .map(|entry| {
            serde_json::json!({
                "t": entry.t,
                "level": entry.level,
                "msg": entry.msg,
            })
        })
        .collect()
}

pub fn pending_count() -> usize {
    queue().lock().unwrap().len()
}

/// 上传成功后移除已确认的条目。
pub fn ack(count: usize) {
    let snapshot = {
        let mut guard = queue().lock().unwrap();
        let count = count.min(guard.len());
        guard.drain(..count);
        guard.clone()
    };
    let _ = crate::secure_store::write_json(crate::secure_store::LOG_QUEUE, &snapshot);
}

/// 立即把内存队列落盘。
pub fn flush_local() {
    let snapshot = queue().lock().unwrap().clone();
    let _ = crate::secure_store::write_json(crate::secure_store::LOG_QUEUE, &snapshot);
}
