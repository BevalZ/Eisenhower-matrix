mod models;
mod db;
mod jevai;

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

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> Result<Settings, String> {
    let key = state.db.get_api_key()?;
    Ok(Settings {
        api_key_configured: key.is_some(),
    })
}

#[tauri::command]
fn set_api_key(key: String, state: tauri::State<AppState>) -> Result<(), String> {
    state.db.set_api_key(&key)
}

#[tauri::command]
async fn classify_task(
    description: String,
    state: tauri::State<'_, AppState>,
) -> Result<ClassificationResult, String> {
    let api_key = state.db.get_api_key()?.ok_or(
        "尚未配置 API Key，请先在设置中填入 TypeSafe AI API Key",
    )?;
    jevai::classify(&api_key, &description).await
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
            classify_task,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
