//! 数据模型与本地持久化。
//!
//! 持久化文件位于桌面 exe 目录或 Android 应用私有目录：identity.json（设备身份，device_id 固定复用
//! ——严禁每次随机，会触发 10121 风控）、session.json（登录态）、config.json
//! （账号/参数）、points_cache.json（点位缓存）。

pub use crate::crypto::header::HeaderIdentity;

pub const HOST: &str = "https://run.gxapp.iydsj.com";
/// 排行榜 / 违规名单域名（信封链与 RUN 相同）。
pub const DISCOVERY: &str = "https://discovery.gxapp.iydsj.com";

/// 登录态（session.json）。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Session {
    #[serde(default)]
    pub uid: i64,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub unid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_weight")]
    pub weight: f64,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub device_id: String,
    /// 登录响应完整业务数据（我的页展示）
    #[serde(default)]
    pub profile: serde_json::Value,
}

fn default_weight() -> f64 {
    68.0
}

impl Session {
    pub fn is_logged_in(&self) -> bool {
        self.uid >= 1 && !self.token.is_empty()
    }
}

/// UI 参数（config.json）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    #[serde(default = "default_f32")]
    pub dist_min: f32,
    #[serde(default = "default_f32")]
    pub dist_max: f32,
    #[serde(default = "default_pace_min")]
    pub pace_min: f32,
    #[serde(default = "default_pace_max")]
    pub pace_max: f32,
    #[serde(default)]
    pub face_check: bool,
    /// 跑步时覆盖轨迹点 bdA；为空表示使用生成器的海拔曲线。
    #[serde(default)]
    pub manual_altitude: Option<f64>,
    /// 跑步时将生成器海拔曲线映射到此范围；为空表示不使用范围覆盖。
    #[serde(default)]
    pub manual_altitude_range: Option<crate::track::altitude::AltitudeRange>,
    /// OSM 路网文件路径（真实道路路由用）。
    #[serde(default)]
    pub osm_path: String,
    /// 路线算法：legacy | road | track。
    #[serde(default = "default_route_mode")]
    pub route_mode: String,
    /// 只经过策略必经点的前 N 个；None=全部，Some(0)=不经过任何必经点。
    #[serde(default)]
    pub must_points_limit: Option<usize>,
}

fn default_route_mode() -> String {
    "legacy".into()
}

fn default_f32() -> f32 {
    1.0
}
fn default_pace_min() -> f32 {
    360.0
}
fn default_pace_max() -> f32 {
    480.0
}

impl Default for Config {
    fn default() -> Self {
        Self {
            dist_min: 1.0,
            dist_max: 1.5,
            pace_min: default_pace_min(),
            pace_max: default_pace_max(),
            // 官方：无抽检记录时 faceCheck=0；本工具不产抽检记录，默认必须是 0
            face_check: false,
            manual_altitude: None,
            manual_altitude_range: None,
            osm_path: String::new(),
            route_mode: default_route_mode(),
            must_points_limit: None,
        }
    }
}

// ── 持久化（平台数据目录，统一加密落盘）────────────────────────

fn exe_dir() -> std::path::PathBuf {
    crate::platform::data_dir()
}

fn read_json<T: serde::de::DeserializeOwned>(name: &str) -> Option<T> {
    crate::secure_store::read_json(name)
}

fn write_json<T: serde::Serialize>(name: &str, value: &T) -> Result<(), String> {
    crate::secure_store::write_json(name, value)
}

/// 加载设备身份；device_id / app_install_time 缺失时生成一次并立即落盘，
/// 此后同一设备全生命周期复用（逐请求漂移会影响设备一致性）。
pub fn load_identity() -> HeaderIdentity {
    load_identity_for_platform(if cfg!(target_os = "android") {
        "android"
    } else {
        "ios"
    })
}

fn load_identity_for_platform(platform: &str) -> HeaderIdentity {
    let mut id: HeaderIdentity = read_json("identity.json").unwrap_or_else(|| {
        let mut identity = HeaderIdentity::default();
        if platform == "android" {
            identity.platform = "android".into();
            // Generic fallback until the user opts in or fills the fields manually.
            identity.device_name = "Android".into();
            identity.os_version = "16".into();
        }
        identity
    });
    let mut dirty = false;
    if id.device_id.is_empty() {
        id.device_id = uuid::Uuid::new_v4().to_string().to_uppercase();
        dirty = true;
    }
    if id.app_install_time <= 0 {
        id.app_install_time =
            crate::crypto::header::HeaderIdentity::fresh_install_time(&id.platform);
        dirty = true;
    }
    if id.mac_address.is_empty() {
        id.mac_address = crate::crypto::header::random_mac();
        dirty = true;
    }
    let before = (id.battery.clone(), id.sensors.clone(), id.gt.clone());
    refresh_derived(&mut id);
    if before.0 != id.battery || before.1 != id.sensors || before.2 != id.gt {
        dirty = true;
    }
    if dirty {
        let _ = save_identity(&id);
    }
    id
}

/// 重算依赖机型的派生档案（电池 / 传感器 / 极验 dev）；设备串 `d` 保留不变。
///
/// 判定"机型变了"用 `gt.dev` 里记录的 model/osv；只缺字段时也会补齐。
pub fn refresh_derived(identity: &mut HeaderIdentity) {
    let model_changed = identity
        .gt
        .as_ref()
        .map(|state| {
            state.dev.model != identity.device_name
                || state.dev.osv != identity.os_version
                // iOS 档案曾误用安卓形态（os/ven/prp），检测到即触发重算迁移
                || (identity.platform == "ios" && state.dev.os != "ios")
        })
        .unwrap_or(true);

    if identity.battery.is_none() || model_changed {
        let generated = crate::battery::generate_for(identity);
        identity.battery = Some(generated);
    }
    if identity.sensors.is_none() || model_changed {
        let generated = crate::sensors::generate_for(identity);
        identity.sensors = Some(generated);
    }
    let device_string = identity
        .gt
        .as_ref()
        .map(|state| state.d.clone())
        .unwrap_or_else(|| {
            crate::geetoken::fresh_device_string(
                &identity.device_id,
                identity.app_install_time.max(0),
            )
        });
    if identity.gt.is_none() || model_changed {
        let dev = crate::geetoken::generate_dev_for(identity);
        identity.gt = Some(crate::geetoken::GtState {
            dev,
            d: device_string,
        });
    }
}

#[cfg(any(target_os = "android", test))]
fn minus_one() -> i64 {
    -1
}

/// Android 读机返回的真实电池字段；读不到的用 -1/0 表示，合并时只覆盖有效值。
#[cfg(any(target_os = "android", test))]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RealBattery {
    #[serde(default)]
    pub capacity_mah: i64,
    #[serde(default)]
    pub voltage_mv: i64,
    #[serde(default = "minus_one")]
    pub level: i64,
    #[serde(default = "minus_one")]
    pub status: i64,
    #[serde(default = "minus_one")]
    pub plugged: i64,
    #[serde(default = "minus_one")]
    pub health: i64,
    #[serde(default)]
    pub temperature_c: f64,
    #[serde(default)]
    pub technology: String,
}

#[cfg(any(target_os = "android", test))]
#[derive(Debug, serde::Deserialize)]
pub struct DeviceInfo {
    pub manufacturer: String,
    pub model: String,
    pub os_version: String,
    /// Android 端由 Java 从 SensorManager 读取的真实清单；桌面端为 None。
    #[serde(default)]
    pub sensors: Option<Vec<crate::sensors::SensorInfo>>,
    /// Android 端读取的真实电池；桌面端为 None。
    #[serde(default)]
    pub battery: Option<RealBattery>,
}

#[cfg(any(target_os = "android", test))]
pub fn identity_with_device_info(
    identity: &HeaderIdentity,
    info: &DeviceInfo,
) -> Result<HeaderIdentity, String> {
    if info.model.trim().is_empty() || info.os_version.trim().is_empty() {
        return Err("未能读取完整的机型和系统版本，请手动填写".into());
    }
    let mut updated = identity.clone();
    updated.platform = "android".into();
    updated.device_name = info.model.trim().into();
    updated.manufacturer = info.manufacturer.trim().into();
    updated.os_version = info.os_version.trim().into();
    // 机型变了，电池档案跟着重算，避免"iPhone 容量配 Android 机型"。
    let generated = crate::battery::generate(
        &updated.device_name,
        &updated.manufacturer,
        &updated.device_id,
        "android",
    );
    updated.battery = Some(generated);
    // Android 读机时 Java 会带上真实电池；有效字段覆盖生成值。
    if let Some(real) = &info.battery {
        let mut merged = updated.battery.clone().unwrap_or_default();
        if real.capacity_mah > 0 {
            merged.capacity_mah = real.capacity_mah as u32;
        }
        if real.voltage_mv > 1000 {
            merged.voltage_mv = real.voltage_mv as u32;
        }
        if (0..=100).contains(&real.level) {
            merged.level = real.level as u8;
        }
        if real.status >= 0 {
            merged.status = real.status as i32;
        }
        if real.plugged >= 0 {
            merged.plugged = real.plugged as i32;
        }
        if real.health > 0 {
            merged.health = real.health as i32;
        }
        if real.temperature_c > 0.0 {
            merged.temperature_c = real.temperature_c as f32;
        }
        if !real.technology.trim().is_empty() {
            merged.technology = real.technology.trim().to_string();
        }
        updated.battery = Some(merged);
    }
    let generated_sensors = crate::sensors::generate(
        &updated.device_name,
        &updated.device_id,
        "android",
    );
    updated.sensors = Some(generated_sensors);
    // Android 读机时 Java 会带上真实传感器清单，优先使用真机数据。
    if let Some(list) = &info.sensors {
        if !list.is_empty() {
            updated.sensors = Some(crate::sensors::SensorProfile {
                sensors: list.clone(),
            });
        }
    }
    // 极验档案跟随新机型重算，但设备串 d 必须保留（否则 fp 变化像换了设备）。
    let device_string = updated
        .gt
        .as_ref()
        .map(|state| state.d.clone())
        .unwrap_or_else(|| {
            crate::geetoken::fresh_device_string(
                &updated.device_id,
                updated.app_install_time.max(0),
            )
        });
    let gt_dev = crate::geetoken::generate_dev_for(&updated);
    updated.gt = Some(crate::geetoken::GtState {
        dev: gt_dev,
        d: device_string,
    });
    Ok(updated)
}

pub fn save_identity(id: &HeaderIdentity) -> Result<(), String> {
    write_json("identity.json", id)
}

pub fn load_session() -> Session {
    read_json("session.json").unwrap_or_default()
}

pub fn save_session(s: &Session) -> Result<(), String> {
    write_json("session.json", s)
}

pub fn clear_session() {
    crate::secure_store::remove("session.json");
}

pub fn load_config() -> Config {
    read_json("config.json").unwrap_or_default()
}

pub fn save_config(c: &Config) -> Result<(), String> {
    write_json("config.json", c)
}

#[cfg(test)]
mod storage_tests {
    use super::*;

    #[test]
    fn android_identity_import_persists_brand_and_reuses_uuid() {
        let directory =
            std::env::temp_dir().join(format!("neko-identity-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        crate::platform::TEST_DATA_DIR.with(|p| *p.borrow_mut() = Some(directory.clone()));
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                crate::platform::TEST_DATA_DIR.with(|p| *p.borrow_mut() = None);
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(directory);
        let first = load_identity_for_platform("android");
        assert_eq!(first.platform, "android");
        assert_ne!(first.device_name, "iPhone");
        assert!(uuid::Uuid::parse_str(&first.device_id).is_ok());
        let again = load_identity_for_platform("android");
        assert_eq!(
            serde_json::to_value(&first).unwrap(),
            serde_json::to_value(&again).unwrap()
        );

        let mut existing = first.clone();
        existing.platform = "ios".into();
        existing.device_name = "Existing manual name".into();
        existing.idfa = "manual-value".into();
        let mut legacy = serde_json::to_value(&existing).unwrap();
        legacy.as_object_mut().unwrap().remove("manufacturer");
        write_json("identity.json", &legacy).unwrap();
        let loaded = load_identity_for_platform("android");
        // 派生档案（电池/传感器/极验 dev）会跟随手动机型重算，其余身份字段必须原样保留。
        assert_eq!(loaded.device_name, existing.device_name);
        assert_eq!(loaded.idfa, existing.idfa);
        assert_eq!(loaded.device_id, existing.device_id);
        assert_eq!(loaded.mac_address, existing.mac_address);
        assert_eq!(loaded.app_install_time, existing.app_install_time);

        let info: DeviceInfo = serde_json::from_str(
            r#"{"manufacturer":"Example","model":"Phone 16","os_version":"16"}"#,
        )
        .unwrap();
        assert_eq!(info.manufacturer, "Example");
        let imported = identity_with_device_info(&loaded, &info).unwrap();
        assert_eq!(imported.platform, "android");
        assert_eq!(imported.device_name, "Phone 16");
        assert_eq!(imported.os_version, "16");
        let mut expected = serde_json::to_value(&existing).unwrap();
        expected["platform"] = "android".into();
        expected["device_name"] = "Phone 16".into();
        expected["manufacturer"] = "Example".into();
        expected["os_version"] = "16".into();
        let expected_battery = crate::battery::generate(
            "Phone 16",
            "Example",
            &existing.device_id,
            "android",
        );
        let expected_sensors = crate::sensors::generate("Phone 16", &existing.device_id, "android");
        expected["battery"] = serde_json::to_value(&expected_battery).unwrap();
        expected["sensors"] = serde_json::to_value(&expected_sensors).unwrap();
        let mut gt_identity = existing.clone();
        gt_identity.platform = "android".into();
        gt_identity.device_name = "Phone 16".into();
        gt_identity.manufacturer = "Example".into();
        gt_identity.os_version = "16".into();
        gt_identity.battery = Some(expected_battery.clone());
        gt_identity.sensors = Some(expected_sensors.clone());
        let expected_gt = crate::geetoken::GtState {
            dev: crate::geetoken::generate_dev_for(&gt_identity),
            d: existing.gt.as_ref().map(|state| state.d.clone()).unwrap_or_default(),
        };
        expected["gt"] = serde_json::to_value(&expected_gt).unwrap();
        assert_eq!(
            serde_json::to_value(&imported).unwrap(),
            expected,
            "import must preserve UUID, MAC, manual IMEI/IDFA, install time and location"
        );
        assert_eq!(
            load_identity().device_name,
            existing.device_name,
            "preview must not save implicitly"
        );
        save_identity(&imported).unwrap();
        let stored: serde_json::Value = read_json("identity.json").unwrap();
        assert_eq!(
            stored.get("manufacturer").and_then(|value| value.as_str()),
            Some("Example"),
            "the consented brand must be written to identity.json"
        );
        let reloaded = load_identity();
        assert_eq!(
            serde_json::to_value(&reloaded).unwrap(),
            serde_json::to_value(&imported).unwrap(),
            "brand and identity must survive a fresh load from disk"
        );
        assert_eq!(reloaded.device_id, first.device_id);
        let (header, _) =
            crate::crypto::header::build_header_for(&reloaded, 0, "", Some(1_700_000_000_000));
        let header: serde_json::Value = serde_json::from_str(&header).unwrap();
        assert_eq!(header["DeviceId"], first.device_id);
        assert_eq!(
            header["deviceName"], "Phone 16",
            "brand must not be prepended to the protocol model field"
        );
        assert_eq!(header["osVersion"], "16");
        assert!(
            header.get("manufacturer").is_none() && header.get("brand").is_none(),
            "persisting local brand metadata must not invent new protocol fields"
        );
    }

    #[test]
    fn incomplete_native_device_info_cannot_replace_identity() {
        let identity = HeaderIdentity::default();
        for (model, os_version) in [("", "16"), ("Phone", " ")] {
            let info = DeviceInfo {
                manufacturer: String::new(),
                model: model.into(),
                os_version: os_version.into(),
                sensors: None,
                battery: None,
            };
            assert!(identity_with_device_info(&identity, &info).is_err());
        }
    }

    #[test]
    fn session_roundtrip_and_logout_use_private_data_directory() {
        let directory = std::env::temp_dir().join(format!("neko-storage-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        crate::platform::TEST_DATA_DIR.with(|p| *p.borrow_mut() = Some(directory.clone()));
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                crate::platform::TEST_DATA_DIR.with(|p| *p.borrow_mut() = None);
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(directory.clone());
        assert_eq!(
            exe_dir(),
            directory,
            "storage must use the injected app directory"
        );
        let session = Session {
            uid: 123,
            token: "test-only-token".into(),
            ..Default::default()
        };
        save_session(&session).unwrap();
        assert!(directory.join("session.json").is_file());
        assert_eq!(load_session().uid, 123);
        assert_eq!(load_session().token, session.token);
        clear_session();
        assert!(!directory.join("session.json").exists());
        assert_eq!(load_session().uid, 0);
    }
}

/// 点位缓存：{ts_ms, points}，TTL 300s（服务端限流 10603：5 分钟 3 次）。
pub const POINTS_TTL_MS: i64 = 300_000;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct PointsCache {
    ts: i64,
    anchor: crate::location::Coordinate,
    points: Vec<serde_json::Value>,
}

/// 只返回与本次锚点相同的缓存。旧版未记录锚点的缓存会自然失效，避免串城市。
pub fn load_points_cache_for(
    anchor: crate::location::Coordinate,
) -> Option<(i64, Vec<serde_json::Value>)> {
    let v: serde_json::Value = read_json("points_cache.json")?;
    let cache: PointsCache = serde_json::from_value(v).ok()?;
    if !cache.anchor.is_near(anchor, 0.0001) {
        return None;
    }
    let ts = cache.ts;
    let pts = cache.points;
    Some((ts, pts))
}

pub fn save_points_cache(
    anchor: crate::location::Coordinate,
    points: &[serde_json::Value],
) -> Result<(), String> {
    let doc = PointsCache {
        ts: crate::crypto::envelope::now_ms(),
        anchor,
        points: points.to_vec(),
    };
    write_json("points_cache.json", &doc)
}

#[cfg(test)]
mod points_cache_tests {
    use super::*;
    #[test]
    fn cache_is_scoped_to_anchor() {
        let cache = PointsCache {
            ts: 1,
            anchor: crate::location::Coordinate::new(39.9, 116.4, 0.0).unwrap(),
            points: vec![],
        };
        let decoded: PointsCache =
            serde_json::from_value(serde_json::to_value(cache).unwrap()).unwrap();
        assert!(decoded.anchor.is_near(
            crate::location::Coordinate::new(39.9, 116.4, 0.0).unwrap(),
            0.0001
        ));
        assert!(!decoded.anchor.is_near(
            crate::location::Coordinate::new(38.9, 121.5, 0.0).unwrap(),
            0.0001
        ));
    }
}

/// 围栏缓存：拉取成功后落盘，UI 预览无网时兜底。
pub fn load_fence_cache() -> Option<Vec<Vec<(f64, f64)>>> {
    let v: serde_json::Value = read_json("fence_cache.json")?;
    serde_json::from_value(v.get("fences")?.clone()).ok()
}

pub fn save_fence_cache(fences: &[Vec<(f64, f64)>]) -> Result<(), String> {
    let doc = serde_json::json!({
        "ts": crate::crypto::envelope::now_ms(),
        "fences": fences,
    });
    write_json("fence_cache.json", &doc)
}
