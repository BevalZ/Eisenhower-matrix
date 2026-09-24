use crate::models::*;

const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

pub fn validate_url(url: &str) -> Result<(), String> {
    let parsed = reqwest::Url::parse(url.trim()).map_err(|_| "WebDAV 地址无效".to_string())?;
    match parsed.scheme() {
        "https" | "http" => {}
        _ => return Err("WebDAV 仅支持 http 或 https".into()),
    }
    if parsed.host_str().is_none() {
        return Err("WebDAV 地址缺少主机名".into());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("请把账号密码填在单独的字段，不要写进地址".into());
    }
    Ok(())
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| format!("无法创建网络客户端: {}", e))
}

/// Upload tasks backup to WebDAV server.
pub async fn upload(
    url: &str,
    username: &str,
    password: &str,
    backup_json: &str,
) -> Result<usize, String> {
    validate_url(url)?;
    let client = client()?;
    let resp = client
        .put(url.trim())
        .basic_auth(username, Some(password))
        .header("Content-Type", "application/json")
        .body(backup_json.to_string())
        .send()
        .await
        .map_err(|e| format!("WebDAV 连接失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("WebDAV 返回错误: {}", resp.status()));
    }
    let tasks: BackupData = serde_json::from_str(backup_json)
        .map_err(|e| format!("本地备份解析失败: {}", e))?;
    Ok(tasks.tasks.len())
}

/// Download tasks backup from WebDAV server.
pub async fn download(url: &str, username: &str, password: &str) -> Result<String, String> {
    validate_url(url)?;
    let client = client()?;
    let resp = client
        .get(url.trim())
        .basic_auth(username, Some(password))
        .send()
        .await
        .map_err(|e| format!("WebDAV 连接失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("WebDAV 返回错误: {}", resp.status()));
    }
    resp.text().await.map_err(|e| e.to_string())
}
