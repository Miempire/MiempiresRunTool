//! 路网下载、缓存与坐标锚点应用。
//!
//! 缓存目录：`<data>/roads/{id}/{version}.osm`（OSM 为公开地图数据，明文缓存）；
//! 缓存索引（含版本/SHA256/城市/中心点）加密保存在 road_meta.json。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::client::{self, ApiError};
use super::model::RoadInfo;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CachedRoad {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub city: String,
    #[serde(default)]
    pub center_lat: f64,
    #[serde(default)]
    pub center_lon: f64,
    #[serde(default)]
    pub coord_sys: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PendingUse {
    pub road_id: String,
    pub run_id: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct RoadCache {
    #[serde(default)]
    pub items: HashMap<String, CachedRoad>,
    /// 上次使用的路网 id（启动时优先展示）。
    #[serde(default)]
    pub last_id: Option<String>,
    /// 尚未成功上报的使用记录（离线补报，按 runId 去重）。
    #[serde(default)]
    pub pending_use: Vec<PendingUse>,
}

pub fn load_cache() -> RoadCache {
    crate::secure_store::read_json(crate::secure_store::ROAD_META).unwrap_or_default()
}

pub fn save_cache(cache: &RoadCache) -> Result<(), String> {
    crate::secure_store::write_json(crate::secure_store::ROAD_META, cache)
}

fn roads_dir() -> PathBuf {
    crate::platform::data_dir().join("roads")
}

/// 本地文件路径（暂存与正式一致，下载完成后由调用方改名为该路径）。
pub fn local_path(road: &RoadInfo) -> PathBuf {
    let version = if road.version.trim().is_empty() {
        "current"
    } else {
        road.version.trim()
    };
    roads_dir().join(&road.id).join(format!("{version}.osm"))
}

fn cached_matches(road: &RoadInfo) -> Option<PathBuf> {
    let path = local_path(road);
    if !path.is_file() {
        return None;
    }
    let meta = load_cache().items.get(&road.id).cloned().unwrap_or_default();
    if meta.version == road.version && !road.sha256.is_empty() {
        if let Ok(actual) = sha256_file(&path) {
            if actual.eq_ignore_ascii_case(&road.sha256) {
                return Some(path);
            }
        }
    }
    Some(path)
}

/// 确保路网已缓存到本地；必要时下载并校验 SHA256。
pub fn ensure(
    road: &RoadInfo,
    token: &str,
    device_id: &str,
    mut progress: impl FnMut(u64, u64),
) -> Result<PathBuf, ApiError> {
    let path = local_path(road);
    if let Some(existing) = cached_matches(road) {
        if let Ok(actual) = sha256_file(&existing) {
            if road.sha256.is_empty() || actual.eq_ignore_ascii_case(&road.sha256) {
                remember(road);
                return Ok(existing);
            }
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| ApiError {
                status: 0,
                code: "io".into(),
                message: format!("创建路网目录失败: {e}"),
            })?;
    }
    client::download_road(token, device_id, &road.id, &path, &mut progress)?;
    if !road.sha256.is_empty() {
        let actual = sha256_file(&path).map_err(|e| ApiError {
            status: 0,
            code: "io".into(),
            message: e,
        })?;
        if !actual.eq_ignore_ascii_case(&road.sha256) {
            let _ = std::fs::remove_file(&path);
            return Err(ApiError {
                status: 0,
                code: "checksum".into(),
                message: "路网文件校验失败，请重试".into(),
            });
        }
    }
    remember(road);
    Ok(path)
}

fn remember(road: &RoadInfo) {
    let mut cache = load_cache();
    cache.items.insert(
        road.id.clone(),
        CachedRoad {
            name: road.name.clone(),
            version: road.version.clone(),
            sha256: road.sha256.clone(),
            city: road.city.clone(),
            center_lat: road.center_lat,
            center_lon: road.center_lon,
            coord_sys: road.coord_sys.clone(),
        },
    );
    cache.last_id = Some(road.id.clone());
    let _ = save_cache(&cache);
}

/// 计算路网锚点（BD09）。不写身份文件，供预览时拉取检查点。
pub fn anchor_bd09(road: &RoadInfo) -> Result<(f64, f64), String> {
    if road.center_lat.abs() < 0.0001 || road.center_lon.abs() < 0.0001 {
        return Err("该路网未配置中心坐标，请联系管理员补充".into());
    }
    let sys = road.coord_sys.trim().to_ascii_uppercase();
    Ok(match sys.as_str() {
        "WGS84" => crate::track::geom::wgs84_to_bd09(road.center_lat, road.center_lon),
        "GCJ02" => crate::track::geom::gcj02_to_bd09(road.center_lat, road.center_lon),
        _ => (road.center_lat, road.center_lon),
    })
}

/// 把服务端下发的城市与中心坐标写入官方身份锚点（BD09）。
pub fn apply_anchor(road: &RoadInfo) -> Result<(), String> {
    let (lat, lon) = anchor_bd09(road)?;
    let mut identity = crate::api::model::load_identity();
    identity.city = if road.city.trim().is_empty() {
        identity.city
    } else {
        road.city.trim().to_string()
    };
    identity.anchor_lat = lat;
    identity.anchor_lon = lon;
    crate::api::model::save_identity(&identity)
}

/// 选定路网：写入 OSM 路径 + 跑步模式 + 城市锚点。
/// `mode` 为 "road"（真实路网路由）或 "track"（跑道绕圈）。
pub fn activate(road: &RoadInfo, path: &Path, mode: &str) -> Result<(), String> {
    let mut config = crate::api::model::load_config();
    config.osm_path = path.to_string_lossy().into_owned();
    config.route_mode = if mode == "road" { "road".into() } else { "track".into() };
    crate::api::model::save_config(&config)?;
    apply_anchor(road)
}

/// 读取并对齐（WGS84 → BD09）路网。
pub fn load_network(path: &Path) -> Result<route_planner::RoadGraph, String> {
    let mut network = crate::track::generate_road::load_network_path(
        path.to_str().ok_or("路网路径包含非法字符")?,
    )?;
    crate::track::generate_road::align_network(&mut network);
    Ok(network)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    client::sha256_file(path)
}

/// 记录一次待上报的使用（写入加密缓存，等待联网补报）。
pub fn queue_use(road_id: &str, run_id: &str, version: &str) {
    let mut cache = load_cache();
    if cache.pending_use.iter().any(|item| item.run_id == run_id) {
        return;
    }
    cache.pending_use.push(PendingUse {
        road_id: road_id.to_string(),
        run_id: run_id.to_string(),
        version: version.to_string(),
    });
    if cache.pending_use.len() > 200 {
        let drop = cache.pending_use.len() - 200;
        cache.pending_use.drain(..drop);
    }
    let _ = save_cache(&cache);
}

pub fn confirm_use(run_id: &str) {
    let mut cache = load_cache();
    let before = cache.pending_use.len();
    cache.pending_use.retain(|item| item.run_id != run_id);
    if cache.pending_use.len() != before {
        let _ = save_cache(&cache);
    }
}

pub fn has_pending_uses() -> bool {
    !load_cache().pending_use.is_empty()
}

/// 依次补报；遇到网络/授权错误立即停止，已成功的从队列移除。
pub fn flush_uses(token: &str, device_id: &str) -> Result<usize, ApiError> {
    let pending = load_cache().pending_use;
    let mut done = 0;
    for item in pending {
        match client::road_use(token, device_id, &item.road_id, &item.run_id, &item.version) {
            Ok(_) => {
                confirm_use(&item.run_id);
                done += 1;
            }
            Err(error) => {
                if error.is_license() || error.is_unauthorized() {
                    return Err(error);
                }
                return Ok(done);
            }
        }
    }
    Ok(done)
}
