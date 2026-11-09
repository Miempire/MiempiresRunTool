//! 服务器地址维护（唯一定义点）。
//!
//! 服务器将来可能更换：客户端只依赖这里的根地址，接口路径见 `endpoints.rs`。

/// 默认服务器根地址（不含末尾斜杠）。
pub const DEFAULT_BASE: &str = "https://jsspace.xyz:8443";

/// 规范化用户输入的地址：去空白/末尾斜杠，缺少协议时补 https。
pub fn normalize(input: &str) -> String {
    let mut value = input.trim().to_string();
    if value.is_empty() {
        return DEFAULT_BASE.into();
    }
    if !value.starts_with("http://") && !value.starts_with("https://") {
        value = format!("https://{value}");
    }
    while value.ends_with('/') {
        value.pop();
    }
    value
}

/// 地址是否可接受（仅允许 http/https，且包含主机名）。
pub fn validate(input: &str) -> Result<String, String> {
    let value = normalize(input);
    let rest = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
        .ok_or_else(|| "地址必须以 http:// 或 https:// 开头".to_string())?;
    if rest.is_empty() || rest.starts_with('/') || rest.contains(' ') {
        return Err("服务器地址格式不正确".into());
    }
    Ok(value)
}
