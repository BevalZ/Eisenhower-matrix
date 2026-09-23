use crate::models::*;

/// Upload tasks backup to WebDAV server.
pub async fn upload(
    url: &str,
    username: &str,
    password: &str,
    backup_json: &str,
) -> Result<usize, String> {
    let client = reqwest::Client::new();
    let resp = client
        .put(url)
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
pub async fn download(
    url: &str,
    username: &str,
    password: &str,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .basic_auth(username, Some(password))
        .send()
        .await
        .map_err(|e| format!("WebDAV 连接失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("WebDAV 返回错误: {}", resp.status()));
    }
    resp.text().await.map_err(|e| e.to_string())
}
