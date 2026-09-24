use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::process::Command;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpSocket};
use tokio::sync::{oneshot, watch};

use crate::db::Db;
use crate::models::*;

const AUTO_SYNC_FIRST: Duration = Duration::from_secs(3);
const AUTO_SYNC_INTERVAL: Duration = Duration::from_secs(45);

static APP: OnceLock<AppHandle> = OnceLock::new();

pub fn set_app(app: AppHandle) {
    let _ = APP.set(app);
}

fn notify_tasks_changed() {
    if let Some(app) = APP.get() {
        let _ = app.emit("tasks-changed", ());
    }
}

const MAX_HEADER: usize = 64 * 1024;
const MAX_BODY: usize = 2 * 1024 * 1024;

pub struct SyncControl {
    running: std::sync::Mutex<Option<Running>>,
    last_error: Arc<std::sync::Mutex<String>>,
}

struct Running {
    addr: String,
    stop: watch::Sender<bool>,
    done: oneshot::Receiver<()>,
}

impl SyncControl {
    pub fn new() -> Self {
        Self {
            running: std::sync::Mutex::new(None),
            last_error: Arc::new(std::sync::Mutex::new(String::new())),
        }
    }

    pub fn status(&self) -> (bool, String, String) {
        let addr = self
            .running
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|running| running.addr.clone()))
            .unwrap_or_default();
        let err = self
            .last_error
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        (!addr.is_empty(), addr, err)
    }

    pub async fn start(&self, db: Arc<Db>) -> Result<String, String> {
        self.stop().await;
        let snapshot = tailscale_status();
        if !snapshot.running || snapshot.ip.is_empty() {
            let message = if snapshot.message.is_empty() {
                "Tailscale 未连接".into()
            } else {
                snapshot.message
            };
            self.set_error(&message);
            return Err(message);
        }
        let port = db.sync_port()?;
        db.require_sync_secret()?;
        let addr: SocketAddr = format!("{}:{port}", snapshot.ip)
            .parse()
            .map_err(|_| "Tailscale 地址无效".to_string())?;
        let socket = TcpSocket::new_v4().map_err(|e| e.to_string())?;
        socket.set_reuseaddr(true).map_err(|e| e.to_string())?;
        socket.bind(addr).map_err(|e| {
            format!("无法监听 {addr}：{e}。请确认 Tailscale 已连接，且端口未被占用。")
        })?;
        let listener = socket.listen(128).map_err(|e| e.to_string())?;
        let (stop_tx, stop_rx) = watch::channel(false);
        let (done_tx, done_rx) = oneshot::channel();
        let server_db = Arc::clone(&db);
        tokio::spawn(async move {
            run_server(listener, server_db, stop_rx).await;
            let _ = done_tx.send(());
        });
        let auto_db = db;
        let mut auto_stop = stop_tx.subscribe();
        let auto_error = Arc::clone(&self.last_error);
        tokio::spawn(async move {
            let mut delay = AUTO_SYNC_FIRST;
            loop {
                tokio::select! {
                    changed = auto_stop.changed() => {
                        if changed.is_err() || *auto_stop.borrow() {
                            break;
                        }
                    }
                    _ = tokio::time::sleep(delay) => {
                        delay = AUTO_SYNC_INTERVAL;
                        if *auto_stop.borrow() {
                            break;
                        }
                        let result = sync_online(&auto_db).await;
                        if *auto_stop.borrow() {
                            break;
                        }
                        match result {
                            Ok(()) => set_shared_error(&auto_error, ""),
                            Err(err) => set_shared_error(&auto_error, &format!("自动同步：{err}")),
                        }
                    }
                }
            }
        });
        let bound = addr.to_string();
        if let Ok(mut guard) = self.running.lock() {
            *guard = Some(Running {
                addr: bound.clone(),
                stop: stop_tx,
                done: done_rx,
            });
        }
        self.set_error("");
        Ok(bound)
    }

    pub async fn stop(&self) {
        let done = {
            let mut guard = match self.running.lock() {
                Ok(guard) => guard,
                Err(_) => return,
            };
            guard.take().map(|running| {
                let _ = running.stop.send(true);
                running.done
            })
        };
        if let Some(done) = done {
            let _ = tokio::time::timeout(Duration::from_secs(2), done).await;
        }
    }

    fn set_error(&self, message: &str) {
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = message.to_string();
        }
    }
}

pub fn tailscale_status() -> TailscaleStatus {
    match tailscale_status_json() {
        Ok(raw) => parse_status(&raw),
        Err(message) => TailscaleStatus {
            running: false,
            hostname: String::new(),
            ip: String::new(),
            peers: Vec::new(),
            message,
        },
    }
}

pub async fn exchange(db: &Db, ip: &str, port: u16) -> Result<usize, String> {
    if !is_tailscale_ipv4(ip) {
        return Err("只能同步 Tailscale 设备地址".into());
    }
    let secret = db.require_sync_secret()?;
    let envelope = SyncEnvelope {
        device_id: db.device_id()?,
        tasks: db.sync_snapshot()?,
    };
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| format!("无法创建网络客户端: {e}"))?;
    let url = format!("http://{ip}:{port}/v1/sync");
    let response = client
        .post(&url)
        .bearer_auth(secret)
        .json(&envelope)
        .send()
        .await
        .map_err(|_| {
            format!("无法连接 {ip}:{port}。请确认对方已开始接受同步，且 Tailscale 在线。")
        })?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if status.as_u16() == 401 {
            return Err("同步密钥不匹配，或对方还没设置密钥".into());
        }
        return Err(format!("对方返回 {status}：{body}"));
    }
    let remote: SyncEnvelope = response
        .json()
        .await
        .map_err(|e| format!("无法解析对方数据: {e}"))?;
    let applied = db.merge_remote(&remote.tasks)?;
    if applied > 0 {
        notify_tasks_changed();
    }
    Ok(applied)
}

async fn sync_online(db: &Db) -> Result<(), String> {
    let status = tailscale_status();
    if !status.running {
        return Err(if status.message.is_empty() {
            "Tailscale 未连接".into()
        } else {
            status.message
        });
    }
    let peers: Vec<_> = status.peers.into_iter().filter(|peer| peer.online).collect();
    if peers.is_empty() {
        return Ok(());
    }
    let port = db.sync_port()?;
    let mut errors = Vec::new();
    for peer in peers {
        if let Err(err) = exchange(db, &peer.ip, port).await {
            errors.push(format!("{}：{err}", peer.hostname));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("；"))
    }
}

fn tailscale_bins() -> Vec<String> {
    let mut bins = vec!["tailscale".to_string()];
    #[cfg(target_os = "macos")]
    {
        bins.push("/Applications/Tailscale.app/Contents/MacOS/Tailscale".into());
        bins.push("/opt/homebrew/bin/tailscale".into());
        bins.push("/usr/local/bin/tailscale".into());
    }
    #[cfg(target_os = "linux")]
    {
        bins.push("/usr/bin/tailscale".into());
        bins.push("/usr/local/bin/tailscale".into());
    }
    #[cfg(target_os = "windows")]
    {
        bins.push(r"C:\Program Files\Tailscale\tailscale.exe".into());
    }
    bins
}

fn tailscale_status_json() -> Result<String, String> {
    let mut last_error = "未找到 Tailscale".to_string();
    for bin in tailscale_bins() {
        match Command::new(&bin).args(["status", "--json"]).output() {
            Ok(output) if output.status.success() => {
                return String::from_utf8(output.stdout)
                    .map_err(|_| "Tailscale 输出不是 UTF-8".to_string());
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                last_error = format!("{bin} 执行失败：{}{}", stderr.trim(), stdout.trim());
            }
            Err(err) => last_error = format!("{bin}：{err}"),
        }
    }
    Err(format!(
        "无法读取 Tailscale 状态。请安装 Tailscale 并登录同一 tailnet。{last_error}"
    ))
}

fn parse_status(raw: &str) -> TailscaleStatus {
    let value: Value = match serde_json::from_str(raw) {
        Ok(value) => value,
        Err(_) => {
            return TailscaleStatus {
                running: false,
                hostname: String::new(),
                ip: String::new(),
                peers: Vec::new(),
                message: "无法解析 Tailscale 状态".into(),
            };
        }
    };
    let state = value.get("BackendState").and_then(Value::as_str).unwrap_or("");
    let self_node = value.get("Self").cloned().unwrap_or(Value::Null);
    let hostname = node_name(&self_node);
    let ip = tailscale_ipv4(self_node.get("TailscaleIPs")).unwrap_or_default();
    let mut peers = Vec::new();
    if let Some(map) = value.get("Peer").and_then(Value::as_object) {
        for peer in map.values() {
            let Some(ip) = tailscale_ipv4(peer.get("TailscaleIPs")) else {
                continue;
            };
            peers.push(TailscalePeer {
                hostname: node_name(peer),
                ip,
                online: peer.get("Online").and_then(Value::as_bool).unwrap_or(false),
                os: peer
                    .get("OS")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            });
        }
    }
    peers.sort_by(|a, b| a.hostname.to_lowercase().cmp(&b.hostname.to_lowercase()));
    let running = state == "Running" && !ip.is_empty();
    let message = if running {
        String::new()
    } else if state.is_empty() {
        "Tailscale 未运行".into()
    } else {
        format!("Tailscale 状态：{state}")
    };
    TailscaleStatus {
        running,
        hostname,
        ip,
        peers,
        message,
    }
}

fn node_name(node: &Value) -> String {
    let host = node.get("HostName").and_then(Value::as_str).unwrap_or("");
    if !host.is_empty() {
        return host.to_string();
    }
    node.get("DNSName")
        .and_then(Value::as_str)
        .unwrap_or("未知设备")
        .trim_end_matches('.')
        .to_string()
}

fn tailscale_ipv4(value: Option<&Value>) -> Option<String> {
    value?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .find(|ip| is_tailscale_ipv4(ip))
        .map(str::to_string)
}

pub fn is_tailscale_ipv4(ip: &str) -> bool {
    ip.parse::<Ipv4Addr>().is_ok_and(is_cgnat)
}

fn is_cgnat(addr: Ipv4Addr) -> bool {
    let octets = addr.octets();
    octets[0] == 100 && (octets[1] & 0b1100_0000) == 0b0100_0000
}

async fn run_server(listener: TcpListener, db: Arc<Db>, mut stop: watch::Receiver<bool>) {
    loop {
        tokio::select! {
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    break;
                }
            }
            accepted = listener.accept() => {
                let Ok((socket, addr)) = accepted else { break };
                let IpAddr::V4(ip) = addr.ip() else { continue };
                if !is_cgnat(ip) {
                    continue;
                }
                let db = Arc::clone(&db);
                tokio::spawn(async move {
                    let _ = handle_socket(socket, db).await;
                });
            }
        }
    }
}

async fn handle_socket(mut socket: tokio::net::TcpStream, db: Arc<Db>) -> Result<(), String> {
    let request = read_request(&mut socket).await?;
    let response = match dispatch(&db, &request).await {
        Ok(body) => http_response("200 OK", "application/json", &body),
        Err(err) => {
            let status = if err == "unauthorized" { "401 Unauthorized" } else { "400 Bad Request" };
            let message = if err == "unauthorized" { "同步密钥不匹配" } else { &err };
            http_response(status, "text/plain; charset=utf-8", message)
        }
    };
    let _ = tokio::time::timeout(Duration::from_secs(15), socket.write_all(response.as_bytes())).await;
    Ok(())
}

async fn dispatch(db: &Db, request: &HttpRequest) -> Result<String, String> {
    if request.method != "POST" || request.path != "/v1/sync" {
        return Err("只接受 POST /v1/sync".into());
    }
    let secret = db.require_sync_secret().map_err(|_| "unauthorized".to_string())?;
    let presented = request
        .authorization
        .strip_prefix("Bearer ")
        .ok_or_else(|| "unauthorized".to_string())?;
    if !secrets_match(presented.trim(), &secret) {
        return Err("unauthorized".into());
    }
    let envelope: SyncEnvelope = serde_json::from_slice(&request.body)
        .map_err(|e| format!("JSON 解析失败: {e}"))?;
    let applied = db.merge_remote(&envelope.tasks)?;
    if applied > 0 {
        notify_tasks_changed();
    }
    let response = SyncEnvelope {
        device_id: db.device_id()?,
        tasks: db.sync_snapshot()?,
    };
    serde_json::to_string(&response).map_err(|e| e.to_string())
}

struct HttpRequest {
    method: String,
    path: String,
    authorization: String,
    body: Vec<u8>,
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> Result<HttpRequest, String> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let header_end = loop {
        let read = tokio::time::timeout(Duration::from_secs(15), socket.read(&mut tmp))
            .await
            .map_err(|_| "读取超时".to_string())?
            .map_err(|e| e.to_string())?;
        if read == 0 {
            return Err("连接已关闭".into());
        }
        buf.extend_from_slice(&tmp[..read]);
        if let Some(index) = find_subsequence(&buf, b"\r\n\r\n") {
            break index;
        }
        if buf.len() > MAX_HEADER {
            return Err("请求头过大".into());
        }
    };
    let header_text = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = header_text.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let mut content_length = 0usize;
    let mut authorization = String::new();
    for line in lines {
        let Some((name, value)) = line.split_once(':') else { continue };
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value.trim().parse().map_err(|_| "Content-Length 无效".to_string())?;
        } else if name.eq_ignore_ascii_case("authorization") {
            authorization = value.trim().to_string();
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err("不支持分块传输".into());
        }
    }
    if content_length > MAX_BODY {
        return Err("同步数据过大".into());
    }
    let body_start = header_end + 4;
    while buf.len() < body_start + content_length {
        let read = tokio::time::timeout(Duration::from_secs(15), socket.read(&mut tmp))
            .await
            .map_err(|_| "读取超时".to_string())?
            .map_err(|e| e.to_string())?;
        if read == 0 {
            return Err("同步数据不完整".into());
        }
        buf.extend_from_slice(&tmp[..read]);
        if buf.len() > body_start + MAX_BODY {
            return Err("同步数据过大".into());
        }
    }
    let body = buf[body_start..body_start + content_length].to_vec();
    Ok(HttpRequest { method, path, authorization, body })
}

fn http_response(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.as_bytes().len()
    )
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

fn set_shared_error(slot: &Arc<std::sync::Mutex<String>>, message: &str) {
    if let Ok(mut guard) = slot.lock() {
        *guard = message.to_string();
    }
}

fn secrets_match(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

