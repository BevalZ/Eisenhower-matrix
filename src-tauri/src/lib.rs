mod models;
mod db;
mod jevai;
mod webdav;
mod sync;

use std::sync::Arc;

use models::*;
use db::Db;
use sync::SyncControl;
use tauri::Manager;

struct AppState {
    db: Arc<Db>,
    sync: Arc<SyncControl>,
}

#[tauri::command]
fn get_tasks(state: tauri::State<AppState>) -> Result<Vec<Task>, String> {
    state.db.get_tasks()
}

#[tauri::command]
fn create_task(input: TaskInput, state: tauri::State<AppState>) -> Result<Task, String> {
    state.db.create_task(&input)
}

#[tauri::command]
fn toggle_task_done(id: i64, state: tauri::State<AppState>) -> Result<(), String> {
    state.db.toggle_done(id)
}

#[tauri::command]
fn delete_task(id: i64, state: tauri::State<AppState>) -> Result<(), String> {
    state.db.delete_task(id)
}

#[tauri::command]
fn move_task(
    id: i64,
    quadrant: i64,
    priority: f64,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    state.db.move_task(id, quadrant, priority)
}

#[tauri::command]
fn clear_done_tasks(state: tauri::State<AppState>) -> Result<(), String> {
    state.db.clear_done()
}

#[tauri::command]
fn get_stats(state: tauri::State<AppState>) -> Result<StatsSummary, String> {
    state.db.get_stats()
}

// ---- Settings ----

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> Result<Settings, String> {
    let api_key = state.db.get_api_key()?;
    let theme = state.db.get_setting("theme")?.unwrap_or_else(|| "light".into());
    let webdav_url = state.db.get_setting("webdav_url")?;
    Ok(Settings {
        api_key_configured: api_key.is_some(),
        theme,
        webdav_configured: webdav_url.is_some(),
    })
}

#[tauri::command]
fn set_api_key(key: String, state: tauri::State<AppState>) -> Result<(), String> {
    state.db.set_api_key(&key)
}

#[tauri::command]
fn set_theme(theme: String, state: tauri::State<AppState>) -> Result<(), String> {
    if theme != "light" && theme != "dark" {
        return Err("主题只能是 light 或 dark".into());
    }
    state.db.set_setting("theme", &theme)
}

#[tauri::command]
fn save_webdav(config: WebdavConfig, state: tauri::State<AppState>) -> Result<(), String> {
    let url = config.url.trim();
    webdav::validate_url(url)?;
    state.db.set_setting("webdav_url", url)?;
    state.db.set_setting("webdav_username", config.username.trim())?;
    state.db.set_setting("webdav_password", &config.password)
}

// ---- AI ----

#[tauri::command]
async fn classify_task(
    description: String,
    state: tauri::State<'_, AppState>,
) -> Result<ClassificationResult, String> {
    let api_key = state.db.get_api_key()?.ok_or(
        "尚未配置 API Key，请先在设置中填入 TypeSafe AI API Key",
    )?;
    let (imp_bias, urg_bias) = state.db.get_calibration_bias()?;
    jevai::classify(&api_key, &description, imp_bias, urg_bias).await
}

#[tauri::command]
fn record_feedback(
    task_title: String,
    ai_importance: f64,
    ai_urgency: f64,
    ai_quadrant: i64,
    user_importance: f64,
    user_urgency: f64,
    user_quadrant: i64,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    state.db.record_feedback(
        &task_title,
        ai_importance,
        ai_urgency,
        ai_quadrant,
        user_importance,
        user_urgency,
        user_quadrant,
    )
}

#[tauri::command]
fn get_learning_stats(state: tauri::State<AppState>) -> Result<LearningStats, String> {
    state.db.get_learning_stats()
}

// ---- Window / Floating Ball ----

#[tauri::command]
fn minimize_to_ball(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(main) = app.get_webview_window("main") {
        main.hide().map_err(|e| e.to_string())?;
    }
    let ball = app.get_webview_window("floating-ball").ok_or("no ball")?;
    ball.show().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn restore_from_ball(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(ball) = app.get_webview_window("floating-ball") {
        ball.hide().map_err(|e| e.to_string())?;
    }
    let main = app.get_webview_window("main").ok_or("no main")?;
    main.show().map_err(|e| e.to_string())?;
    main.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

// ---- Import / Export ----

#[tauri::command]
fn export_data(state: tauri::State<AppState>) -> Result<String, String> {
    state.db.export_all()
}

#[tauri::command]
fn import_data(json: String, state: tauri::State<AppState>) -> Result<usize, String> {
    state.db.import_all(&json)
}

// ---- WebDAV Sync ----

#[tauri::command]
async fn sync_to_webdav(state: tauri::State<'_, AppState>) -> Result<SyncResult, String> {
    let url = state.db.get_setting("webdav_url")?.ok_or("未配置 WebDAV")?;
    let username = state.db.get_setting("webdav_username")?.unwrap_or_default();
    let password = state.db.get_setting("webdav_password")?.unwrap_or_default();
    let backup = state.db.export_all()?;
    let count = webdav::upload(&url, &username, &password, &backup).await?;
    Ok(SyncResult {
        success: true,
        message: format!("已同步 {} 个任务到 WebDAV", count),
        task_count: count,
    })
}

#[tauri::command]
async fn restore_from_webdav(state: tauri::State<'_, AppState>) -> Result<SyncResult, String> {
    let url = state.db.get_setting("webdav_url")?.ok_or("未配置 WebDAV")?;
    let username = state.db.get_setting("webdav_username")?.unwrap_or_default();
    let password = state.db.get_setting("webdav_password")?.unwrap_or_default();
    let json = webdav::download(&url, &username, &password).await?;
    let count = state.db.import_all(&json)?;
    Ok(SyncResult {
        success: true,
        message: format!("已从 WebDAV 恢复 {} 个任务", count),
        task_count: count,
    })
}


#[tauri::command]
fn tailscale_status() -> TailscaleStatus {
    sync::tailscale_status()
}

#[tauri::command]
fn peer_sync_status(state: tauri::State<AppState>) -> Result<PeerSyncStatus, String> {
    let (listening, address, last_error) = state.sync.status();
    Ok(PeerSyncStatus {
        listening,
        address,
        port: state.db.sync_port()?,
        secret: state.db.get_setting("sync_secret")?.unwrap_or_default(),
        device_id: state.db.device_id()?,
        last_error,
    })
}

#[tauri::command]
async fn save_peer_sync(
    config: PeerSyncConfig,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state.db.save_sync_config(&config.secret, config.port)?;
    if state.sync.status().0 {
        state.sync.start(Arc::clone(&state.db)).await?;
    }
    Ok(())
}

#[tauri::command]
async fn set_peer_sync_listening(
    enabled: bool,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    if enabled {
        let addr = state.sync.start(Arc::clone(&state.db)).await?;
        state.db.set_setting("sync_listen", "1")?;
        Ok(addr)
    } else {
        state.sync.stop().await;
        state.db.set_setting("sync_listen", "0")?;
        Ok(String::new())
    }
}

#[tauri::command]
async fn sync_with_peer(
    ip: String,
    state: tauri::State<'_, AppState>,
) -> Result<SyncResult, String> {
    let applied = sync_one(&state, &ip).await?;
    Ok(sync_message(applied))
}

#[tauri::command]
async fn sync_all_peers(state: tauri::State<'_, AppState>) -> Result<SyncResult, String> {
    let status = sync::tailscale_status();
    if !status.running {
        return Err(if status.message.is_empty() { "Tailscale 未连接".into() } else { status.message });
    }
    let peers: Vec<_> = status.peers.into_iter().filter(|peer| peer.online).collect();
    if peers.is_empty() {
        return Err("没有其他在线的 Tailscale 设备".into());
    }
    let mut lines = Vec::new();
    let mut total = 0usize;
    let mut failures = 0usize;
    for peer in peers {
        match sync_one(&state, &peer.ip).await {
            Ok(count) => {
                total += count;
                lines.push(format!("{}：本机更新 {} 条", peer.hostname, count));
            }
            Err(err) => {
                failures += 1;
                lines.push(format!("{}：失败（{err}）", peer.hostname));
            }
        }
    }
    Ok(SyncResult {
        success: failures == 0,
        message: lines.join("\n"),
        task_count: total,
    })
}

async fn sync_one(state: &AppState, ip: &str) -> Result<usize, String> {
    let status = sync::tailscale_status();
    if !status.running {
        return Err(if status.message.is_empty() { "Tailscale 未连接".into() } else { status.message });
    }
    if status.ip == ip {
        return Err("不能和本机同步".into());
    }
    if !status.peers.iter().any(|peer| peer.ip == ip && peer.online) {
        return Err("这台设备不在当前 Tailscale 网络，或当前不在线".into());
    }
    sync::exchange(&state.db, ip, state.db.sync_port()?).await
}

fn sync_message(applied: usize) -> SyncResult {
    SyncResult {
        success: true,
        message: if applied == 0 {
            "同步完成，两边已经一致".into()
        } else {
            format!("同步完成，本机更新了 {applied} 条任务")
        },
        task_count: applied,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            db: Arc::new(Db::open().expect("failed to open database")),
            sync: Arc::new(SyncControl::new()),
        })
        .invoke_handler(tauri::generate_handler![
            get_tasks,
            create_task,
            toggle_task_done,
            delete_task,
            move_task,
            clear_done_tasks,
            get_stats,
            get_settings,
            set_api_key,
            set_theme,
            save_webdav,
            classify_task,
            record_feedback,
            get_learning_stats,
            export_data,
            import_data,
            sync_to_webdav,
            restore_from_webdav,
            minimize_to_ball,
            restore_from_ball,
            tailscale_status,
            peer_sync_status,
            save_peer_sync,
            set_peer_sync_listening,
            sync_with_peer,
            sync_all_peers,
        ])
        .setup(|app| {
            sync::set_app(app.handle().clone());
            let state = app.state::<AppState>();
            let enabled = state.db.get_setting("sync_listen").unwrap_or(None);
            if enabled.as_deref() == Some("1") {
                let db = Arc::clone(&state.db);
                let sync = Arc::clone(&state.sync);
                tauri::async_runtime::spawn(async move {
                    let _ = sync.start(db).await;
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if window.label() == "main" {
                    window.app_handle().exit(0);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
