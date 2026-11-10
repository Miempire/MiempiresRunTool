//! 客户端本地设置：服务器地址（可被用户/运维替换）与主题偏好。
//!
//! 地址与接口路径彻底分离：地址只在 `server` 中维护，接口路径只在 `endpoints` 中维护。

pub mod endpoints;
pub mod server;

/// 本地设置（加密落盘于 app_settings.json）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    /// 服务器根地址，例如 https://jsspace.xyz:8443。
    #[serde(default)]
    pub server_base: String,
    /// 主题：sakura（粉色）| ocean（蓝色）。
    #[serde(default = "default_theme")]
    pub theme: String,
    /// 是否已经展示过设备信息说明。
    #[serde(default)]
    pub device_notice_shown: bool,
    /// 上次使用的距离（km）。
    #[serde(default = "default_distance")]
    pub last_dist_km: f64,
    /// 上次使用的配速（秒/km）。
    #[serde(default = "default_pace")]
    pub last_pace_sec: f64,
    /// 上次使用的配速范围下限（更快，秒/km）。
    #[serde(default = "default_pace_min")]
    pub last_pace_min_sec: f64,
    /// 上次使用的配速范围上限（更慢，秒/km）。
    #[serde(default = "default_pace_max")]
    pub last_pace_max_sec: f64,
    /// 上次使用的基准海拔（米）。
    #[serde(default = "default_altitude")]
    pub last_altitude_m: f64,
    /// 上次自定义的配速曲线（x=进度 0~1，y=秒/km）。
    #[serde(default)]
    pub last_pace_curve: Vec<(f64, f64)>,
    /// 上次自定义的海拔曲线（x=进度 0~1，y=米）。
    #[serde(default)]
    pub last_altitude_curve: Vec<(f64, f64)>,
    /// 上次选择的路网 id。
    #[serde(default)]
    pub last_road_id: Option<String>,
}

fn default_distance() -> f64 {
    2.0
}

fn default_theme() -> String {
    "sakura".into()
}

fn default_pace() -> f64 {
    360.0
}

fn default_pace_min() -> f64 {
    330.0
}

fn default_pace_max() -> f64 {
    390.0
}

fn default_altitude() -> f64 {
    20.0
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server_base: server::DEFAULT_BASE.into(),
            theme: default_theme(),
            device_notice_shown: false,
            last_dist_km: default_distance(),
            last_pace_sec: default_pace(),
            last_pace_min_sec: default_pace_min(),
            last_pace_max_sec: default_pace_max(),
            last_altitude_m: default_altitude(),
            last_pace_curve: Vec::new(),
            last_altitude_curve: Vec::new(),
            last_road_id: None,
        }
    }
}

pub fn load() -> Settings {
    let mut settings: Settings = crate::secure_store::read_json(crate::secure_store::APP_SETTINGS)
        .unwrap_or_default();
    if settings.server_base.trim().is_empty() {
        settings.server_base = server::DEFAULT_BASE.into();
    }
    settings.server_base = server::normalize(&settings.server_base);
    if settings.theme != "ocean" && settings.theme != "sakura" {
        settings.theme = default_theme();
    }
    settings
}

pub fn save(settings: &Settings) -> Result<(), String> {
    crate::secure_store::write_json(crate::secure_store::APP_SETTINGS, settings)
}

/// 当前服务器根地址。
pub fn base_url() -> String {
    let settings = load();
    settings.server_base
}

/// 拼接完整接口地址。`path` 必须以 `/` 开头。
pub fn url(path: &str) -> String {
    format!("{}{}", base_url(), path)
}
