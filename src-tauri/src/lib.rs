mod models;
mod db;
mod jevai;
mod webdav;

use models::*;
use db::Db;

struct AppState {
    db: Db,
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
    state.db.set_setting("theme", &theme)
}

#[tauri::command]
fn save_webdav(config: WebdavConfig, state: tauri::State<AppState>) -> Result<(), String> {
    state.db.set_setting("webdav_url", &config.url)?;
    state.db.set_setting("webdav_username", &config.username)?;
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            db: Db::open().expect("failed to open database"),
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
