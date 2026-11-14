pub mod config;
pub mod discord;
pub mod models;

use config::ConfigManager;
use discord::client::DiscordClient;
use discord::engine::ReactorEngine;
use models::{Account, BotSettings, CategoryConfig, Config, DiscordChannel, DiscordGuild, EmojiSenderSettings, EngineStatus};
use parking_lot::RwLock;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

pub struct AppState {
    pub config_mgr: ConfigManager,
    pub engine: ReactorEngine,
    pub log_history: Arc<RwLock<Vec<String>>>,
}

#[tauri::command]
fn get_config(state: State<'_, AppState>) -> Config {
    state.config_mgr.get_config()
}

#[tauri::command]
fn get_active_account(state: State<'_, AppState>) -> Option<Account> {
    state.config_mgr.get_active_account()
}

#[tauri::command]
fn set_active_account(account_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.config_mgr.set_active_account(account_id)
}

#[tauri::command]
fn add_or_update_account(
    name: String,
    token: String,
    account_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    state.config_mgr.add_or_update_account(name, token, account_id)
}

#[tauri::command]
fn delete_account(account_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.config_mgr.delete_account(&account_id)
}

#[tauri::command]
fn update_bot_settings(settings: BotSettings, state: State<'_, AppState>) -> Result<(), String> {
    state.config_mgr.update_bot_settings(settings)
}

#[tauri::command]
fn update_categories(categories: Vec<CategoryConfig>, state: State<'_, AppState>) -> Result<(), String> {
    state.config_mgr.update_categories(categories)
}

#[tauri::command]
fn update_emoji_filters(
    whitelist: Vec<String>,
    blacklist: Vec<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.config_mgr.update_emoji_filters(whitelist, blacklist)
}

#[tauri::command]
fn update_emoji_sender(settings: EmojiSenderSettings, state: State<'_, AppState>) -> Result<(), String> {
    state.config_mgr.update_emoji_sender(settings)
}

#[tauri::command]
fn reset_progress(state: State<'_, AppState>) -> Result<(), String> {
    state.config_mgr.reset_progress()
}

#[tauri::command]
fn get_status(state: State<'_, AppState>) -> EngineStatus {
    state.engine.get_status(&state.config_mgr)
}

#[tauri::command]
fn get_logs(state: State<'_, AppState>) -> Vec<String> {
    state.log_history.read().clone()
}

#[tauri::command]
fn clear_logs(state: State<'_, AppState>) {
    state.log_history.write().clear();
}

#[tauri::command]
async fn fetch_user_guilds(state: State<'_, AppState>) -> Result<Vec<DiscordGuild>, String> {
    let account = state
        .config_mgr
        .get_active_account()
        .ok_or_else(|| "Chưa chọn tài khoản hoặc chưa có Token!".to_string())?;

    if account.token.trim().is_empty() {
        return Err("Token của tài khoản đang chọn bị trống!".to_string());
    }

    let client = DiscordClient::new(account.token)?;
    client.get_user_guilds().await
}

#[tauri::command]
async fn fetch_guild_channels(guild_id: String, state: State<'_, AppState>) -> Result<Vec<DiscordChannel>, String> {
    let account = state
        .config_mgr
        .get_active_account()
        .ok_or_else(|| "Chưa chọn tài khoản hoặc chưa có Token!".to_string())?;

    if account.token.trim().is_empty() {
        return Err("Token của tài khoản đang chọn bị trống!".to_string());
    }

    let client = DiscordClient::new(account.token)?;
    client.get_guild_channels(&guild_id).await
}

#[tauri::command]
async fn start_bot(app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let account = state
        .config_mgr
        .get_active_account()
        .ok_or_else(|| "Chưa chọn tài khoản hoặc chưa có Token!".to_string())?;

    if account.token.trim().is_empty() {
        return Err("Token trống! Vui lòng nhập Discord Token.".to_string());
    }

    let client = DiscordClient::new(account.token.clone())?;

    let log_history = state.log_history.clone();
    let app_handle_clone = app_handle.clone();

    let on_log = Arc::new(move |msg: String| {
        {
            let mut logs = log_history.write();
            if logs.len() > 300 {
                logs.remove(0);
            }
            logs.push(msg.clone());
        }
        let _ = app_handle_clone.emit("log", msg);
    });

    state
        .engine
        .start(client, account.name, state.config_mgr.clone(), on_log)
        .await;

    Ok(())
}

#[tauri::command]
fn stop_bot(state: State<'_, AppState>) {
    state.engine.stop();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Thư mục dữ liệu ứng dụng (chuẩn di động và desktop)
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("data"));

            let config_mgr = ConfigManager::new(data_dir);
            let engine = ReactorEngine::new();
            let log_history = Arc::new(RwLock::new(vec!["Hệ thống sẵn sàng...".to_string()]));

            app.manage(AppState {
                config_mgr,
                engine,
                log_history,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            get_active_account,
            set_active_account,
            add_or_update_account,
            delete_account,
            update_bot_settings,
            update_categories,
            update_emoji_filters,
            update_emoji_sender,
            reset_progress,
            get_status,
            get_logs,
            clear_logs,
            fetch_user_guilds,
            fetch_guild_channels,
            start_bot,
            stop_bot,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
