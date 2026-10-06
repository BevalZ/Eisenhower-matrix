mod models;
mod db;
mod http;
mod jevai;
mod openai;
mod secrets;

mod webdav;
mod sync;

use std::path::PathBuf;
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
fn update_task(input: TaskUpdate, state: tauri::State<AppState>) -> Result<(), String> {
    state.db.update_task(&input)
}

#[tauri::command]
fn reorder_tasks(moves: Vec<TaskMove>, state: tauri::State<AppState>) -> Result<(), String> {
    state.db.reorder_tasks(&moves)
}

#[tauri::command]
fn clear_done_tasks(state: tauri::State<AppState>) -> Result<Vec<i64>, String> {
    state.db.clear_done()
}

#[tauri::command]
fn restore_tasks(ids: Vec<i64>, state: tauri::State<AppState>) -> Result<usize, String> {
    state.db.restore_tasks(&ids)
}

#[tauri::command]
fn get_stats(state: tauri::State<AppState>) -> Result<StatsSummary, String> {
    state.db.get_stats()
}

// ---- Settings ----

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> Result<Settings, String> {
    let theme = state.db.get_setting("theme")?.unwrap_or_else(|| "light".into());
    let webdav_url = state.db.get_setting("webdav_url")?;
    let ai = ai_settings(&state.db)?;
    let jevai_key = secrets::is_set(&state.db, secrets::API_KEY)?;
    let openai_key = secrets::is_set(&state.db, secrets::OPENAI_API_KEY)?;
    Ok(Settings {
        api_key_configured: if ai.0 == "openai" { !ai.1.is_empty() && !ai.2.is_empty() } else { jevai_key },
        theme,
        webdav_configured: webdav_url.is_some(),
        ai_provider: ai.0,
        ai_base_url: ai.1,
        ai_model: ai.2,
        jevai_key_configured: jevai_key,
        openai_key_configured: openai_key,
        secrets_stored_in_plaintext: secrets::stored_in_plaintext(&state.db)?,
    })
}

/// (provider, base_url, model)
fn ai_settings(db: &Db) -> Result<(String, String, String), String> {
    Ok((
        db.get_setting("ai_provider")?.unwrap_or_else(|| "jevai".into()),
        db.get_setting("ai_base_url")?.unwrap_or_default(),
        db.get_setting("ai_model")?.unwrap_or_default(),
    ))
}

#[tauri::command]
fn save_ai_config(config: AiConfig, state: tauri::State<AppState>) -> Result<(), String> {
    match config.provider.as_str() {
        "jevai" => {}
        "openai" => {
            let url = config.base_url.trim();
            let parsed = reqwest::Url::parse(url).map_err(|_| "接口地址无效".to_string())?;
            if !matches!(parsed.scheme(), "http" | "https") {
                return Err("接口地址需要以 http:// 或 https:// 开头".into());
            }
            if config.model.trim().is_empty() {
                return Err("请填写模型名称".into());
            }
            state.db.set_setting("ai_base_url", url)?;
            state.db.set_setting("ai_model", config.model.trim())?;
            if !config.api_key.trim().is_empty() {
                secrets::set(&state.db, secrets::OPENAI_API_KEY, config.api_key.trim())?;
            }
        }
        _ => return Err("未知的 AI 服务".into()),
    }
    state.db.set_setting("ai_provider", &config.provider)
}


#[tauri::command]
fn set_api_key(key: String, state: tauri::State<AppState>) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("API Key 不能为空".into());
    }
    secrets::set(&state.db, secrets::API_KEY, key)
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
    // An empty password field means "keep the saved one", so re-saving the URL does not wipe it.
    if config.password.is_empty() {
        return Ok(());
    }
    secrets::set(&state.db, secrets::WEBDAV_PASSWORD, &config.password)
}

#[tauri::command]
fn get_webdav_config(state: tauri::State<AppState>) -> Result<WebdavInfo, String> {
    Ok(WebdavInfo {
        url: state.db.get_setting("webdav_url")?.unwrap_or_default(),
        username: state.db.get_setting("webdav_username")?.unwrap_or_default(),
        has_password: secrets::is_set(&state.db, secrets::WEBDAV_PASSWORD)?,
    })
}

// ---- AI ----

#[tauri::command]
async fn classify_task(
    description: String,
    state: tauri::State<'_, AppState>,
) -> Result<ClassificationResult, String> {
    let (imp_bias, urg_bias) = state.db.get_calibration_bias()?;
    let (provider, base_url, model) = ai_settings(&state.db)?;
    if provider == "openai" {
        if base_url.is_empty() || model.is_empty() {
            return Err("请先在设置中填写 OpenAI 兼容接口地址和模型".into());
        }
        let key = secrets::get(&state.db, secrets::OPENAI_API_KEY)?;
        return openai::classify(&base_url, key.as_deref(), &model, &description, imp_bias, urg_bias).await;
    }
    let api_key = secrets::get(&state.db, secrets::API_KEY)?.ok_or(
        "尚未配置 API Key，请先在设置中填入 TypeSafe AI API Key",
    )?;
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

/// Which platform the UI is running on; the board hides desktop-only affordances (the
/// floating ball, Tailscale peer sync) when this says "android" or "ios".
#[tauri::command]
fn platform() -> String {
    if cfg!(target_os = "android") {
        "android".into()
    } else if cfg!(target_os = "ios") {
        "ios".into()
    } else if cfg!(target_os = "windows") {
        "windows".into()
    } else if cfg!(target_os = "macos") {
        "macos".into()
    } else {
        "linux".into()
    }
}

// ---- Window / Floating Ball ----

#[tauri::command]
fn minimize_to_ball(app: tauri::AppHandle) -> Result<(), String> {
    if cfg!(any(target_os = "android", target_os = "ios")) {
        return Err("手机端没有悬浮球".into());
    }
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

/// Save a JSON backup through the native save dialog. Blob downloads are not reliable in
/// every platform WebView. Returns the chosen path, or None if the user cancelled.
#[tauri::command]
async fn export_to_file(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let json = state.db.export_all()?;
    let name = format!("eisenhower-backup-{}.json", chrono::Local::now().format("%Y-%m-%d"));
    // The dialog blocks until the user answers and the write is plain disk I/O; both belong
    // on a blocking thread, not on a runtime worker shared with the rest of the commands.
    tauri::async_runtime::spawn_blocking(move || save_backup(app, name, json))
        .await
        .map_err(|e| format!("导出失败: {e}"))?
}

fn save_backup(app: tauri::AppHandle, name: String, json: String) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let Some(picked) = app
        .dialog()
        .file()
        .add_filter("JSON", &["json"])
        .set_file_name(name)
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("写入文件失败: {e}"))?;
    Ok(Some(path.display().to_string()))
}

#[tauri::command]
fn import_data(json: String, state: tauri::State<AppState>) -> Result<usize, String> {
    state.db.import_all(&json)
}

// ---- WebDAV Sync ----

/// Saved WebDAV connection details: (url, username, password).
fn webdav_credentials(state: &AppState) -> Result<(String, String, String), String> {
    let url = state.db.get_setting("webdav_url")?.ok_or("未配置 WebDAV")?;
    let username = state.db.get_setting("webdav_username")?.unwrap_or_default();
    let password = secrets::get(&state.db, secrets::WEBDAV_PASSWORD)?.unwrap_or_default();
    Ok((url, username, password))
}

#[tauri::command]
async fn sync_to_webdav(state: tauri::State<'_, AppState>) -> Result<SyncResult, String> {
    let (url, username, password) = webdav_credentials(&state)?;
    // Download → merge by uid (last write wins) → upload the merged result, so two
    // devices sharing one WebDAV file don't overwrite each other.
    let remote = webdav::download(&url, &username, &password).await?;
    let (incoming, schema) = db::parse_webdav_payload(remote.as_deref())?;
    let applied = state.db.merge_remote(&incoming, schema)?;
    let tasks = state.db.sync_snapshot()?;
    let uploaded = tasks.iter().filter(|t| t.deleted_at.is_none()).count();
    let envelope = SyncEnvelope {
        device_id: state.db.device_id()?,
        tasks,
        schema: SYNC_SCHEMA,
    };
    let body = serde_json::to_string(&envelope).map_err(|e| e.to_string())?;
    webdav::upload(&url, &username, &password, body).await?;
    Ok(SyncResult {
        success: true,
        message: if remote.is_none() {
            format!("WebDAV 上还没有数据，已上传 {uploaded} 个任务")
        } else {
            format!("同步完成：本机更新了 {applied} 条，上传了 {uploaded} 个任务")
        },
        task_count: applied,
    })
}

#[tauri::command]
async fn restore_from_webdav(state: tauri::State<'_, AppState>) -> Result<SyncResult, String> {
    let (url, username, password) = webdav_credentials(&state)?;
    let json = webdav::download(&url, &username, &password)
        .await?
        .ok_or("WebDAV 上还没有备份文件")?;
    let count = state.db.restore_webdav(&json)?;
    Ok(SyncResult {
        success: true,
        message: format!("已从 WebDAV 恢复 {} 个任务", count),
        task_count: count,
    })
}


#[tauri::command]
async fn tailscale_status() -> TailscaleStatus {
    sync::tailscale_status().await
}

#[tauri::command]
fn peer_sync_status(state: tauri::State<AppState>) -> Result<PeerSyncStatus, String> {
    let (listening, address, last_error) = state.sync.status();
    Ok(PeerSyncStatus {
        listening,
        address,
        port: state.db.sync_port()?,
        secret_set: state.db.sync_secret_configured()?,
        device_id: state.db.device_id()?,
        last_error,
    })
}

/// Reads the shared sync secret back for the settings page. Kept separate from
/// `peer_sync_status` so the secret is only handed over when the user asks to see it.
#[tauri::command]
fn reveal_sync_secret(state: tauri::State<AppState>) -> Result<String, String> {
    state.db.require_sync_secret()
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
    // One status read serves the whole round: spawning `tailscale status` per peer is slow.
    let status = sync::tailscale_status().await;
    let applied = sync_one(&state, &ip, &status).await?;
    Ok(sync_message(applied))
}

#[tauri::command]
async fn sync_all_peers(state: tauri::State<'_, AppState>) -> Result<SyncResult, String> {
    let status = sync::tailscale_status().await;
    if !status.running {
        return Err(if status.message.is_empty() { "Tailscale 未连接".into() } else { status.message.clone() });
    }
    let peers: Vec<_> = status.peers.iter().filter(|peer| peer.online).cloned().collect();
    if peers.is_empty() {
        return Err("没有其他在线的 Tailscale 设备".into());
    }
    let mut lines = Vec::new();
    let mut total = 0usize;
    let mut failures = 0usize;
    for peer in peers {
        match sync_one(&state, &peer.ip, &status).await {
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

async fn sync_one(
    state: &AppState,
    ip: &str,
    status: &TailscaleStatus,
) -> Result<usize, String> {
    if !status.running {
        return Err(if status.message.is_empty() {
            "Tailscale 未连接".into()
        } else {
            status.message.clone()
        });
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

/// Directory that holds `tasks.db`.
///
/// Desktop keeps `dirs`' historical location so an existing install still finds its tasks
/// after this version; mobile has no `dirs` support, so it uses the app's private data
/// directory that Tauri derives from the Android package identifier.
fn data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    if let Some(base) = dirs::data_dir() {
        return Ok(base.join("eisenhower-matrix"));
    }
    app.path()
        .app_data_dir()
        .map_err(|e| format!("无法确定应用数据目录: {e}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_tasks,
            create_task,
            toggle_task_done,
            delete_task,
            update_task,
            reorder_tasks,
            clear_done_tasks,
            restore_tasks,
            get_stats,
            get_settings,
            set_api_key,
            save_ai_config,
            set_theme,
            save_webdav,
            get_webdav_config,
            classify_task,
            record_feedback,
            get_learning_stats,
            export_data,
            export_to_file,
            import_data,
            sync_to_webdav,
            restore_from_webdav,
            minimize_to_ball,
            restore_from_ball,
            tailscale_status,
            peer_sync_status,
            reveal_sync_secret,
            save_peer_sync,
            set_peer_sync_listening,
            sync_with_peer,
            sync_all_peers,
            platform,
        ])
        .setup(|app| {
            // The database is opened here rather than while building the app, because the
            // mobile data directory is only known once the app handle exists.
            let dir = data_dir(app.handle())?;
            let db = Arc::new(Db::open(dir.join("tasks.db"))?);
            sync::set_app(app.handle().clone());
            let sync = Arc::new(SyncControl::new());
            app.manage(AppState {
                db: Arc::clone(&db),
                sync: Arc::clone(&sync),
            });
            // Moving old plaintext secrets may wait on the OS keyring; keep it off the UI thread.
            let migrate_db = Arc::clone(&db);
            std::thread::spawn(move || secrets::migrate(&migrate_db));
            let enabled = db.get_setting("sync_listen").unwrap_or(None);
            if enabled.as_deref() == Some("1") {
                let listen_db = Arc::clone(&db);
                tauri::async_runtime::spawn(async move {
                    let _ = sync.start(listen_db).await;
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
