//! 软件账号登录态（加密持久化）。

use super::model::{LicenseInfo, LoginResult};

pub const SESSION_FILE: &str = crate::secure_store::SOFT_SESSION;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SoftSession {
    pub token: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub nickname: String,
    #[serde(default)]
    pub device_name: String,
    /// pending | active | expired | disabled
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub expires_on: Option<String>,
    #[serde(default)]
    pub days_left: i64,
    #[serde(default)]
    pub saved_at: i64,
}

impl SoftSession {
    pub fn load() -> Option<Self> {
        let session: SoftSession = crate::secure_store::read_json(SESSION_FILE)?;
        if session.token.is_empty() {
            return None;
        }
        Some(session)
    }

    pub fn save(&self) -> Result<(), String> {
        crate::secure_store::write_json(SESSION_FILE, self)
    }

    pub fn clear() {
        crate::secure_store::remove(SESSION_FILE);
    }

    pub fn from_login(login: &LoginResult) -> Self {
        Self {
            token: login.token.clone(),
            account_id: login.account.id.clone(),
            username: login.account.username.clone(),
            nickname: login.account.nickname.clone(),
            device_name: login.account.device_name.clone(),
            status: login.status.clone(),
            expires_on: login.expires_on.clone(),
            days_left: login.days_left,
            saved_at: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn apply_license(&mut self, license: &LicenseInfo) {
        self.status = license.status.clone();
        self.expires_on = license.expires_on.clone();
        self.days_left = license.days_left;
    }

    pub fn is_active(&self) -> bool {
        self.status == "active"
    }

    pub fn status_text(&self) -> String {
        match self.status.as_str() {
            "active" => format!("已授权 · 剩余 {} 天", self.days_left.max(0)),
            "pending" => "待管理员开通".into(),
            "expired" => "使用期限已过".into(),
            "disabled" => "账号已停用".into(),
            _ => "授权状态未知".into(),
        }
    }

    pub fn display_name(&self) -> String {
        if !self.nickname.trim().is_empty() {
            self.nickname.clone()
        } else {
            self.username.clone()
        }
    }
}

/// 软件账号的设备标识：复用官方身份的设备 UUID（全生命周期稳定）。
pub fn device_id() -> String {
    crate::api::model::load_identity().device_id
}

/// 设备展示名（品牌 型号），静默读取失败时退化为 Android。
pub fn device_name() -> String {
    let identity = crate::api::model::load_identity();
    if identity.device_name.trim().is_empty() {
        "Android".into()
    } else {
        identity.device_name
    }
}
