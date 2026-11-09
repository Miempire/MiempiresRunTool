//! 软件服务器 HTTP 客户端（Bearer + X-Device-Id）。
//!
//! 所有请求都走 `config::url`，因此更换服务器只需改本地设置。

use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use super::model::{LicenseInfo, LoginResult, ReleaseInfo, RoadInfo, SoftAccount};
use crate::config::endpoints;

#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: u16,
    pub code: String,
    pub message: String,
}

impl ApiError {
    fn transport(message: impl Into<String>) -> Self {
        Self {
            status: 0,
            code: "network".into(),
            message: message.into(),
        }
    }

    pub fn is_unauthorized(&self) -> bool {
        self.status == 401
    }

    /// 授权类错误（未开通/已过期/停用）。
    pub fn is_license(&self) -> bool {
        self.code.starts_with("license_") || self.code == "account_disabled"
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.message.is_empty() {
            write!(f, "HTTP {}", self.status)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
}

fn user_agent() -> String {
    format!(
        "MiempiresRunTool/{} (Android)",
        crate::platform::version_name()
    )
}

#[derive(serde::Deserialize)]
struct Envelope {
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    data: Option<Value>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

fn decode(status: u16, text: &str) -> Result<Value, ApiError> {
    let envelope: Envelope = serde_json::from_str(text).map_err(|_| ApiError {
        status,
        code: "bad_response".into(),
        message: format!("服务器返回异常（HTTP {status}）"),
    })?;
    if envelope.ok {
        Ok(envelope.data.unwrap_or(Value::Null))
    } else {
        Err(ApiError {
            status,
            code: envelope.error.unwrap_or_default(),
            message: envelope.message.unwrap_or_default(),
        })
    }
}

/// 发起一次请求；`body` 为 None 时使用 GET 语义。
fn send(
    method: &str,
    path: &str,
    token: Option<&str>,
    device_id: &str,
    body: Option<&Value>,
) -> ApiResult<Value> {
    let mut request = agent()
        .request(method, &crate::config::url(path))
        .set("Accept", "application/json")
        .set("User-Agent", &user_agent())
        .set("X-Device-Id", device_id);
    if let Some(token) = token {
        request = request.set("Authorization", &format!("Bearer {token}"));
    }
    let response = match body {
        Some(body) => request
            .set("Content-Type", "application/json")
            .send_string(&body.to_string()),
        None => request.call(),
    };
    match response {
        Ok(response) => {
            let status = response.status();
            let text = response
                .into_string()
                .map_err(|e| ApiError::transport(format!("读取响应失败: {e}")))?;
            decode(status, &text)
        }
        Err(ureq::Error::Status(status, response)) => {
            let text = response.into_string().unwrap_or_default();
            Err(decode(status, &text).err().unwrap_or(ApiError {
                status,
                code: String::new(),
                message: format!("请求失败（HTTP {status}）"),
            }))
        }
        Err(ureq::Error::Transport(error)) => {
            Err(ApiError::transport(format!("无法连接服务器: {error}")))
        }
    }
}

pub fn register(
    username: &str,
    password: &str,
    nickname: &str,
    device_id: &str,
    device_name: &str,
    app_version: &str,
) -> ApiResult<String> {
    let body = serde_json::json!({
        "username": username,
        "password": password,
        "nickname": nickname,
        "deviceId": device_id,
        "deviceName": device_name,
        "appVersion": app_version,
    });
    let data = send("POST", endpoints::REGISTER, None, device_id, Some(&body))?;
    Ok(data
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("注册成功，请联系管理员开通")
        .to_string())
}

pub fn login(
    username: &str,
    password: &str,
    device_id: &str,
    device_name: &str,
) -> ApiResult<LoginResult> {
    let body = serde_json::json!({
        "username": username,
        "password": password,
        "deviceId": device_id,
        "deviceName": device_name,
    });
    let data = send("POST", endpoints::LOGIN, None, device_id, Some(&body))?;
    serde_json::from_value(data).map_err(|e| ApiError {
        status: 0,
        code: "bad_response".into(),
        message: format!("登录响应解析失败: {e}"),
    })
}

fn license_from(data: &Value) -> LicenseInfo {
    LicenseInfo {
        status: data
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        expires_on: data
            .get("expiresOn")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        days_left: data.get("daysLeft").and_then(|v| v.as_i64()).unwrap_or(0),
    }
}

pub fn me(token: &str, device_id: &str) -> ApiResult<(SoftAccount, LicenseInfo)> {
    let data = send("GET", endpoints::ME, Some(token), device_id, None)?;
    let account: SoftAccount =
        serde_json::from_value(data.get("account").cloned().unwrap_or(Value::Null)).unwrap_or_default();
    Ok((account, license_from(&data)))
}

pub fn logout(token: &str, device_id: &str) {
    let _ = send("POST", endpoints::LOGOUT, Some(token), device_id, None);
}

pub fn license_check(token: &str, device_id: &str) -> ApiResult<LicenseInfo> {
    let data = send("GET", endpoints::LICENSE_CHECK, Some(token), device_id, None)?;
    Ok(license_from(&data))
}

pub fn logs_batch(
    token: &str,
    device_id: &str,
    app_version: &str,
    entries: &[Value],
) -> ApiResult<()> {
    let body = serde_json::json!({
        "appVersion": app_version,
        "entries": entries,
    });
    send("POST", endpoints::LOGS_BATCH, Some(token), device_id, Some(&body))?;
    Ok(())
}

pub fn roads(token: &str, device_id: &str) -> ApiResult<Vec<RoadInfo>> {
    let data = send("GET", endpoints::ROADS, Some(token), device_id, None)?;
    serde_json::from_value(data).map_err(|e| ApiError {
        status: 0,
        code: "bad_response".into(),
        message: format!("路网列表解析失败: {e}"),
    })
}

/// 上报一次路网使用（服务端按 runId 去重）。
pub fn road_use(
    token: &str,
    device_id: &str,
    road_id: &str,
    run_id: &str,
    version: &str,
) -> ApiResult<(bool, i64)> {
    let body = serde_json::json!({ "runId": run_id, "version": version });
    let data = send(
        "POST",
        &endpoints::road_use(road_id),
        Some(token),
        device_id,
        Some(&body),
    )?;
    Ok((
        data.get("deduped").and_then(|v| v.as_bool()).unwrap_or(false),
        data.get("usageCount").and_then(|v| v.as_i64()).unwrap_or(0),
    ))
}

pub fn update_latest(token: &str, device_id: &str) -> ApiResult<Option<ReleaseInfo>> {
    let data = send("GET", endpoints::UPDATE_LATEST, Some(token), device_id, None)?;
    if data.is_null() {
        return Ok(None);
    }
    serde_json::from_value(data).map(Some).map_err(|e| ApiError {
        status: 0,
        code: "bad_response".into(),
        message: format!("更新信息解析失败: {e}"),
    })
}

/// 流式下载到临时文件，成功后改名。`url` 可为绝对地址。
pub fn download(
    url: &str,
    token: &str,
    device_id: &str,
    dest: &Path,
    mut progress: impl FnMut(u64, u64),
) -> ApiResult<()> {
    let full = if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        crate::config::url(url)
    };
    let response = agent()
        .get(&full)
        .set("User-Agent", &user_agent())
        .set("Authorization", &format!("Bearer {token}"))
        .set("X-Device-Id", device_id)
        .call();
    let response = match response {
        Ok(response) => response,
        Err(ureq::Error::Status(status, response)) => {
            let text = response.into_string().unwrap_or_default();
            return Err(decode(status, &text).err().unwrap_or(ApiError {
                status,
                code: String::new(),
                message: format!("下载失败（HTTP {status}）"),
            }));
        }
        Err(ureq::Error::Transport(error)) => {
            return Err(ApiError::transport(format!("下载失败: {error}")))
        }
    };
    let total = response
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| ApiError::transport(format!("创建目录失败: {e}")))?;
    }
    let tmp = dest.with_extension("part");
    let mut file =
        std::fs::File::create(&tmp).map_err(|e| ApiError::transport(format!("写入文件失败: {e}")))?;
    let mut reader = response.into_reader();
    let mut buffer = [0u8; 64 * 1024];
    let mut done = 0u64;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|e| ApiError::transport(format!("下载中断: {e}")))?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .map_err(|e| ApiError::transport(format!("写入文件失败: {e}")))?;
        done += read as u64;
        progress(done, total);
    }
    file.flush().ok();
    drop(file);
    if dest.exists() {
        let _ = std::fs::remove_file(dest);
    }
    std::fs::rename(&tmp, dest).map_err(|e| ApiError::transport(format!("保存文件失败: {e}")))
}

/// 下载已选路网到 dest。
pub fn download_road(
    token: &str,
    device_id: &str,
    road_id: &str,
    dest: &Path,
    progress: impl FnMut(u64, u64),
) -> ApiResult<()> {
    download(&endpoints::road_download(road_id), token, device_id, dest, progress)
}

/// 文件 SHA-256（小写十六进制）。
pub fn sha256_file(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(path).map_err(|e| format!("读取文件失败: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| format!("读取文件失败: {e}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    Ok(out)
}
