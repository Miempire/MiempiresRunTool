//! 软件服务器接口路径（与地址分离，地址见 `server.rs`）。
//!
//! 服务端实现：d:\service\Endpoints\SoftApi.cs，统一前缀 /api/soft/v1。

pub const PREFIX: &str = "/api/soft/v1";

// ── 账号与授权 ────────────────────────────────────────────────
pub const REGISTER: &str = "/api/soft/v1/auth/register";
pub const LOGIN: &str = "/api/soft/v1/auth/login";
pub const ME: &str = "/api/soft/v1/auth/me";
pub const LOGOUT: &str = "/api/soft/v1/auth/logout";
pub const LICENSE_CHECK: &str = "/api/soft/v1/license/check";

// ── 日志 ─────────────────────────────────────────────────────
pub const LOGS_BATCH: &str = "/api/soft/v1/logs/batch";

// ── 路网 ─────────────────────────────────────────────────────
pub const ROADS: &str = "/api/soft/v1/roads";

pub fn road_use(id: &str) -> String {
    format!("/api/soft/v1/roads/{}/use", encode(id))
}

pub fn road_download(id: &str) -> String {
    format!("/api/soft/v1/roads/{}/download", encode(id))
}

// ── 更新 ─────────────────────────────────────────────────────
pub const UPDATE_LATEST: &str = "/api/soft/v1/update/latest";

/// 更新地址由服务端下发（绝对地址），这里只在必要时兜底拼接。
pub fn update_download(version: &str) -> String {
    format!("/api/soft/v1/update/download/{}", encode(version))
}

/// 最小化 URL 编码（路径段只允许 ASCII 安全字符）。
pub fn encode(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}
