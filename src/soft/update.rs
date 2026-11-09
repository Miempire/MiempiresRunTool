//! 客户端更新：向服务器查询最新 APK，下载后交给系统安装器。

use std::path::PathBuf;

use super::client::{self, ApiError};
use super::model::ReleaseInfo;

/// 比较版本号（只比较首个数字段，忽略 -android.x 后缀）。
pub fn is_newer(remote: &str, local: &str) -> bool {
    let parse = |value: &str| -> Vec<u64> {
        value
            .split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .take(3)
            .filter_map(|s| s.parse::<u64>().ok())
            .collect()
    };
    let remote_parts = parse(remote);
    let local_parts = parse(local);
    if remote_parts.is_empty() {
        return false;
    }
    for index in 0..3 {
        let r = *remote_parts.get(index).unwrap_or(&0);
        let l = *local_parts.get(index).unwrap_or(&0);
        if r != l {
            return r > l;
        }
    }
    false
}

pub fn check(token: &str, device_id: &str) -> Result<Option<ReleaseInfo>, ApiError> {
    let latest = client::update_latest(token, device_id)?;
    let local = crate::platform::version_name();
    Ok(latest.filter(|release| is_newer(&release.version, &local)))
}

/// 下载 APK 到应用私有目录，返回文件路径。
///
/// 固定保存为 `<filesDir>/update.apk`：系统安装器通过 ApkProvider 只暴露该文件。
pub fn download(
    release: &ReleaseInfo,
    token: &str,
    device_id: &str,
    progress: impl FnMut(u64, u64),
) -> Result<PathBuf, ApiError> {
    let dest = crate::platform::data_dir().join("update.apk");
    let version = if release.version.trim().is_empty() {
        "latest".to_string()
    } else {
        release.version.trim().to_string()
    };
    let url = if release.url.trim().is_empty() {
        crate::config::endpoints::update_download(&version)
    } else {
        release.url.clone()
    };
    client::download(&url, token, device_id, &dest, progress)?;
    if !release.sha256.trim().is_empty() {
        let actual = client::sha256_file(&dest).map_err(|message| ApiError {
            status: 0,
            code: "io".into(),
            message,
        })?;
        if !actual.eq_ignore_ascii_case(release.sha256.trim()) {
            let _ = std::fs::remove_file(&dest);
            return Err(ApiError {
                status: 0,
                code: "checksum".into(),
                message: "安装包校验失败，请重试".into(),
            });
        }
    }
    Ok(dest)
}
