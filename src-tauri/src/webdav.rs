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

fn client() -> Result<&'static reqwest::Client, String> {
    crate::http::client_no_redirect()
}

fn status_error(status: reqwest::StatusCode) -> String {
    let hint = match status.as_u16() {
        401 | 403 => "，请检查账号和密码（坚果云等服务需使用「应用专用密码」，不是登录密码）",
        404 | 409 => "，请确认地址中的文件夹已在网盘里创建",
        301 | 302 | 307 | 308 => "，服务器要求跳转，请填写最终地址（常见于 http 应改为 https）",
        507 => "，网盘空间不足",
        500..=599 => "，服务器暂时出错，请稍后再试",
        _ => "",
    };
    format!("WebDAV 返回错误: {}{}", status, hint)
}

/// Upload a sync payload to the WebDAV server.
pub async fn upload(url: &str, username: &str, password: &str, body: String) -> Result<(), String> {
    validate_url(url)?;
    let client = client()?;
    let resp = client
        .put(url.trim())
        .timeout(TIMEOUT)
        .basic_auth(username, Some(password))
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("WebDAV 连接失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(status_error(resp.status()));
    }
    Ok(())
}

/// Download the payload from the WebDAV server; `None` if the file doesn't exist yet.
pub async fn download(url: &str, username: &str, password: &str) -> Result<Option<String>, String> {
    validate_url(url)?;
    let client = client()?;
    let resp = client
        .get(url.trim())
        .timeout(TIMEOUT)
        .basic_auth(username, Some(password))
        .send()
        .await
        .map_err(|e| format!("WebDAV 连接失败: {}", e))?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !resp.status().is_success() {
        return Err(status_error(resp.status()));
    }
    resp.text().await.map(Some).map_err(|e| e.to_string())
}
