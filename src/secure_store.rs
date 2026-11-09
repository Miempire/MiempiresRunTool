//! 本地敏感数据加密存储。
//!
//! Android：由 `MainActivity` 使用 Android Keystore（AES-256-GCM）加解密，
//! 密钥不出安全硬件；桌面调试：使用数据目录内随机密钥文件做 AES-256-CBC +
//! HMAC-SHA256（仅用于开发自测，正式产物只有 Android）。
//!
//! 文件格式：`MRT1` 魔数 + 平台密文块。解密失败视为数据损坏，调用方按
//! “未登录/未配置”处理，不会把明文写回磁盘。

use std::path::PathBuf;

use cbc::cipher::{block_padding::Pkcs7, BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
use sha2::{Digest, Sha256};

type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

const MAGIC: &[u8; 4] = b"MRT1";

pub const APP_SETTINGS: &str = "app_settings.json";
pub const SOFT_SESSION: &str = "soft_session.json";
pub const LOG_QUEUE: &str = "log_queue.json";
pub const ROAD_META: &str = "road_meta.json";

fn data_dir() -> PathBuf {
    crate::platform::data_dir()
}

fn path_of(name: &str) -> PathBuf {
    data_dir().join(name)
}

pub fn exists(name: &str) -> bool {
    path_of(name).is_file()
}

pub fn remove(name: &str) {
    let _ = std::fs::remove_file(path_of(name));
}

/// 加密并原子替换写入。
pub fn write_bytes(name: &str, plaintext: &[u8]) -> Result<(), String> {
    let sealed = seal(plaintext)?;
    let mut blob = Vec::with_capacity(sealed.len() + MAGIC.len());
    blob.extend_from_slice(MAGIC);
    blob.extend_from_slice(&sealed);
    let path = path_of(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &blob).map_err(|e| format!("写入 {} 失败: {e}", tmp.display()))?;
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    std::fs::rename(&tmp, &path).map_err(|e| format!("保存 {} 失败: {e}", path.display()))
}

/// 读取并解密；文件不存在或损坏返回 None。
pub fn read_bytes(name: &str) -> Option<Vec<u8>> {
    let blob = std::fs::read(path_of(name)).ok()?;
    if blob.len() < MAGIC.len() + 16 || &blob[..MAGIC.len()] != MAGIC {
        return None;
    }
    open(&blob[MAGIC.len()..]).ok()
}

pub fn write_json<T: serde::Serialize>(name: &str, value: &T) -> Result<(), String> {
    let json = serde_json::to_vec(value).map_err(|e| format!("序列化 {name} 失败: {e}"))?;
    write_bytes(name, &json)
}

pub fn read_json<T: serde::de::DeserializeOwned>(name: &str) -> Option<T> {
    let bytes = read_bytes(name)?;
    serde_json::from_slice(&bytes).ok()
}

// ── Android：Keystore 加解密 ──────────────────────────────────

#[cfg(target_os = "android")]
fn seal(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    crate::android::secure_encrypt(plaintext)
}

#[cfg(target_os = "android")]
fn open(ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    crate::android::secure_decrypt(ciphertext)
}

// ── 桌面调试：本地密钥文件 + AES-256-CBC + HMAC-SHA256 ─────────

#[cfg(not(target_os = "android"))]
fn desktop_keys() -> Result<([u8; 32], [u8; 32]), String> {
    use rand::RngCore;
    let path = data_dir().join("secure.key");
    let key = match std::fs::read(&path) {
        Ok(bytes) if bytes.len() == 32 => {
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            key
        }
        _ => {
            let mut key = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut key);
            std::fs::create_dir_all(data_dir()).map_err(|e| format!("创建目录失败: {e}"))?;
            std::fs::write(&path, key).map_err(|e| format!("写入本地密钥失败: {e}"))?;
            key
        }
    };
    let mut enc = Sha256::new();
    enc.update(key);
    enc.update(b"|enc");
    let mut enc_key = [0u8; 32];
    enc_key.copy_from_slice(&enc.finalize());
    let mut mac = Sha256::new();
    mac.update(key);
    mac.update(b"|mac");
    let mut mac_key = [0u8; 32];
    mac_key.copy_from_slice(&mac.finalize());
    Ok((enc_key, mac_key))
}

#[cfg(not(target_os = "android"))]
fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut block = [0u8; 64];
    if key.len() > 64 {
        let digest = Sha256::digest(key);
        block[..32].copy_from_slice(&digest);
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(block.iter().map(|b| b ^ 0x36).collect::<Vec<u8>>());
    inner.update(data);
    let inner = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(block.iter().map(|b| b ^ 0x5c).collect::<Vec<u8>>());
    outer.update(inner);
    let out = outer.finalize();
    let mut tag = [0u8; 32];
    tag.copy_from_slice(&out);
    tag
}

#[cfg(not(target_os = "android"))]
fn seal(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    use rand::RngCore;
    let (enc_key, mac_key) = desktop_keys()?;
    let mut iv = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut iv);
    let ct = Aes256CbcEnc::new((&enc_key).into(), (&iv).into())
        .encrypt_padded_vec::<Pkcs7>(plaintext);
    let mut body = Vec::with_capacity(16 + ct.len());
    body.extend_from_slice(&iv);
    body.extend_from_slice(&ct);
    let tag = hmac_sha256(&mac_key, &body);
    body.extend_from_slice(&tag);
    Ok(body)
}

#[cfg(not(target_os = "android"))]
fn open(ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    if ciphertext.len() < 16 + 16 + 32 {
        return Err("密文长度不正确".into());
    }
    let (body, tag) = ciphertext.split_at(ciphertext.len() - 32);
    let (enc_key, mac_key) = desktop_keys()?;
    let expect = hmac_sha256(&mac_key, body);
    if expect.as_slice() != tag {
        return Err("密文校验失败".into());
    }
    let iv: &[u8; 16] = body[..16].try_into().map_err(|_| "IV 长度不正确".to_string())?;
    Aes256CbcDec::new((&enc_key).into(), iv.into())
        .decrypt_padded_vec::<Pkcs7>(&body[16..])
        .map_err(|e| format!("解密失败: {e}"))
}
