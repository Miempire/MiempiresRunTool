//! 软件服务器数据结构（与 d:\service\Endpoints\SoftApi.cs 的 JSON 对齐）。

/// 软件账号信息。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoftAccount {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub nickname: String,
    #[serde(default)]
    pub device_name: String,
}

/// 登录响应。
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResult {
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub account: SoftAccount,
    /// pending | active | expired | disabled
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub expires_on: Option<String>,
    #[serde(default)]
    pub days_left: i64,
}

/// 授权状态。
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseInfo {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub expires_on: Option<String>,
    #[serde(default)]
    pub days_left: i64,
}

/// 路网（客户端可见字段；服务端不返回上传者账号）。
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadInfo {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub city: String,
    #[serde(default)]
    pub center_lat: f64,
    #[serde(default)]
    pub center_lon: f64,
    /// BD09 | WGS84 | GCJ02
    #[serde(default)]
    pub coord_sys: String,
    #[serde(default)]
    pub radius: Option<f64>,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub size: i64,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub updated_at: String,
    /// 投稿者署名（展示用，不是用户名）。
    #[serde(default)]
    pub author_name: String,
    #[serde(default)]
    pub usage_count: i64,
}

impl RoadInfo {
    /// 署名展示：留空时显示匿名跑者。
    pub fn author(&self) -> String {
        let name = self.author_name.trim();
        if name.is_empty() {
            "匿名跑者".into()
        } else {
            name.to_string()
        }
    }

    /// 大小展示（MB，保留 1 位）。
    pub fn size_text(&self) -> String {
        format!("{:.1} MB", self.size as f64 / 1_048_576.0)
    }
}

/// 客户端更新信息。
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub version_code: i64,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub size: i64,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub published_at: String,
}
