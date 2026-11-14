#[path = "../../src-tauri/src/models.rs"]
pub mod models;
#[path = "../../src-tauri/src/config.rs"]
pub mod config;
#[path = "../../src-tauri/src/discord/mod.rs"]
pub mod discord;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Json, Router,
};
use config::ConfigManager;
use discord::client::DiscordClient;
use discord::engine::ReactorEngine;
use futures_util::stream::Stream;
use models::{Account, BotSettings, CategoryConfig, EmojiSenderSettings, TokenCheck};
use parking_lot::RwLock;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::sleep;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

#[derive(Clone)]
pub struct ServerState {
    pub config_mgr: ConfigManager,
    pub engine: Arc<ReactorEngine>,
    pub log_history: Arc<RwLock<Vec<String>>>,
    pub log_tx: broadcast::Sender<String>,
}

async fn handle_healthz() -> &'static str {
    "OK"
}

async fn handle_get_status(State(state): State<ServerState>) -> Json<serde_json::Value> {
    let status = state.engine.get_status(&state.config_mgr);
    Json(serde_json::to_value(status).unwrap_or(serde_json::Value::Null))
}

async fn handle_get_config(State(state): State<ServerState>) -> Json<serde_json::Value> {
    let cfg = state.config_mgr.get_config();
    Json(serde_json::to_value(cfg).unwrap_or(serde_json::Value::Null))
}

async fn handle_get_logs(State(state): State<ServerState>) -> Json<Vec<String>> {
    let logs = state.log_history.read().clone();
    Json(logs)
}

async fn handle_logs_stream(
    State(state): State<ServerState>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.log_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|res| match res {
        Ok(msg) => Some(Ok(Event::default().data(msg))),
        Err(_) => None,
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn handle_invoke(
    Path(cmd): Path<String>,
    State(state): State<ServerState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    match cmd.as_str() {
        "get_config" => {
            let cfg = state.config_mgr.get_config();
            Ok(Json(serde_json::to_value(cfg).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?))
        }
        "get_active_account" => {
            let acc = state.config_mgr.get_active_account();
            Ok(Json(serde_json::to_value(acc).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?))
        }
        "set_active_account" => {
            let account_id = payload
                .get("accountId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| (StatusCode::BAD_REQUEST, "Thiếu accountId".to_string()))?;
            state
                .config_mgr
                .set_active_account(account_id.to_string())
                .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
            Ok(Json(serde_json::Value::Null))
        }
        "add_or_update_account" => {
            let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("Tài khoản").to_string();
            let token = payload.get("token").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let account_id = payload.get("accountId").and_then(|v| v.as_str()).map(|s| s.to_string());
            let id = state
                .config_mgr
                .add_or_update_account(name, token, account_id)
                .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
            Ok(Json(serde_json::Value::String(id)))
        }
        "set_account_enabled" => {
            let account_id = payload
                .get("accountId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| (StatusCode::BAD_REQUEST, "Thiếu accountId".to_string()))?;
            let enabled = payload.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
            state
                .config_mgr
                .set_account_enabled(account_id, enabled)
                .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
            Ok(Json(serde_json::Value::Null))
        }
        "delete_account" => {
            let account_id = payload
                .get("accountId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| (StatusCode::BAD_REQUEST, "Thiếu accountId".to_string()))?;
            state
                .config_mgr
                .delete_account(account_id)
                .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
            Ok(Json(serde_json::Value::Null))
        }
        "check_token" => {
            let token = payload.get("token").and_then(|v| v.as_str()).unwrap_or("");
            let check = DiscordClient::check_token(token).await;
            Ok(Json(serde_json::to_value(check).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?))
        }
        "update_bot_settings" => {
            let settings: BotSettings = serde_json::from_value(
                payload.get("settings").cloned().unwrap_or_else(|| payload.clone()),
            )
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("Cài đặt không hợp lệ: {}", e)))?;
            state
                .config_mgr
                .update_bot_settings(settings)
                .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
            Ok(Json(serde_json::Value::Null))
        }
        "update_categories" => {
            let categories: Vec<CategoryConfig> = serde_json::from_value(
                payload.get("categories").cloned().unwrap_or_else(|| payload.clone()),
            )
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("Danh mục không hợp lệ: {}", e)))?;
            state
                .config_mgr
                .update_categories(categories)
                .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
            Ok(Json(serde_json::Value::Null))
        }
        "update_emoji_filters" => {
            let whitelist: Vec<String> = payload
                .get("whitelist")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let blacklist: Vec<String> = payload
                .get("blacklist")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            state
                .config_mgr
                .update_emoji_filters(whitelist, blacklist)
                .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
            Ok(Json(serde_json::Value::Null))
        }
        "update_emoji_sender" => {
            let settings: EmojiSenderSettings = serde_json::from_value(
                payload.get("settings").cloned().unwrap_or_else(|| payload.clone()),
            )
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("Emoji sender không hợp lệ: {}", e)))?;
            state
                .config_mgr
                .update_emoji_sender(settings)
                .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
            Ok(Json(serde_json::Value::Null))
        }
        "reset_progress" => {
            state
                .config_mgr
                .reset_progress()
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            Ok(Json(serde_json::Value::Null))
        }
        "get_status" => {
            let status = state.engine.get_status(&state.config_mgr);
            Ok(Json(serde_json::to_value(status).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?))
        }
        "get_logs" => {
            let logs = state.log_history.read().clone();
            Ok(Json(serde_json::to_value(logs).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?))
        }
        "clear_logs" => {
            state.log_history.write().clear();
            Ok(Json(serde_json::Value::Null))
        }
        "start_bot" => {
            let accounts = state.config_mgr.get_running_accounts();
            if accounts.is_empty() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Chưa bật tài khoản nào! Hãy bật ít nhất một tài khoản ở tab Tài khoản.".to_string(),
                ));
            }

            let meter = state.engine.meter();
            let mut pairs = Vec::new();
            for acc in accounts {
                if acc.token.trim().is_empty() {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        format!("Token của “{}” đang trống! Vui lòng nhập Discord Token.", acc.name),
                    ));
                }
                let client = DiscordClient::new(acc.token.trim().to_string())
                    .map_err(|e| (StatusCode::BAD_REQUEST, e))?
                    .with_meter(meter.clone());
                pairs.push((acc, client));
            }

            let log_history = state.log_history.clone();
            let log_tx = state.log_tx.clone();

            let on_log = Arc::new(move |msg: String| {
                println!("{}", msg);
                {
                    let mut logs = log_history.write();
                    if logs.len() > 300 {
                        logs.remove(0);
                    }
                    logs.push(msg.clone());
                }
                let _ = log_tx.send(msg);
            });

            state
                .engine
                .start(pairs, state.config_mgr.clone(), on_log)
                .await;

            Ok(Json(serde_json::Value::Null))
        }
        "stop_bot" => {
            state.engine.stop();
            Ok(Json(serde_json::Value::Null))
        }
        _ => Err((StatusCode::NOT_FOUND, format!("Không tìm thấy lệnh: {}", cmd))),
    }
}

fn spawn_keep_alive() {
    tokio::spawn(async {
        // Render tự động cấp RENDER_EXTERNAL_URL (ví dụ: https://my-bot.onrender.com)
        // Hoặc người dùng có thể cấu hình SELF_PING_URL
        let ping_url = std::env::var("RENDER_EXTERNAL_URL")
            .or_else(|_| std::env::var("SELF_PING_URL"))
            .ok();

        if let Some(mut base_url) = ping_url {
            base_url = base_url.trim_end_matches('/').to_string();
            let target = format!("{}/healthz", base_url);
            println!("[Keep-Alive] Đã kích hoạt cơ chế tự ping chống ngủ cho Render Free: {}", target);

            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_default();

            loop {
                // Ping mỗi 10 phút (Render Free ngủ sau 15 phút không nhận HTTP request)
                sleep(Duration::from_secs(600)).await;
                match client.get(&target).send().await {
                    Ok(resp) => {
                        println!("[Keep-Alive] Ping máy chủ thành công (HTTP {})", resp.status());
                    }
                    Err(err) => {
                        eprintln!("[Keep-Alive] Ping không thành công: {}", err);
                    }
                }
            }
        } else {
            println!("[Keep-Alive] Không phát hiện RENDER_EXTERNAL_URL / SELF_PING_URL.");
        }
    });
}

fn spawn_auto_start(state: ServerState) {
    if std::env::var("AUTO_START").map(|v| v == "true" || v == "1").unwrap_or(false) {
        tokio::spawn(async move {
            sleep(Duration::from_secs(2)).await;
            println!("[Auto-Start] Đang tự động khởi chạy Bot theo biến AUTO_START...");
            let accounts = state.config_mgr.get_running_accounts();
            if accounts.is_empty() {
                eprintln!("[Auto-Start] Chưa có tài khoản nào được bật.");
                return;
            }
            let meter = state.engine.meter();
            let mut pairs = Vec::new();
            for acc in accounts {
                if !acc.token.trim().is_empty() {
                    if let Ok(client) = DiscordClient::new(acc.token.trim().to_string()) {
                        pairs.push((acc, client.with_meter(meter.clone())));
                    }
                }
            }
            if !pairs.is_empty() {
                let log_history = state.log_history.clone();
                let log_tx = state.log_tx.clone();
                let on_log = Arc::new(move |msg: String| {
                    println!("{}", msg);
                    {
                        let mut logs = log_history.write();
                        if logs.len() > 300 {
                            logs.remove(0);
                        }
                        logs.push(msg.clone());
                    }
                    let _ = log_tx.send(msg);
                });
                state.engine.start(pairs, state.config_mgr.clone(), on_log).await;
            }
        });
    }
}

#[tokio::main]
async fn main() {
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(10000);

    let data_dir = std::env::var("DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data"));

    let config_mgr = ConfigManager::new(data_dir);
    let engine = Arc::new(ReactorEngine::new());
    let log_history = Arc::new(RwLock::new(vec!["Discord Reactor Server đã khởi động...".to_string()]));
    let (log_tx, _) = broadcast::channel::<String>(512);

    let state = ServerState {
        config_mgr,
        engine,
        log_history,
        log_tx,
    };

    spawn_keep_alive();
    spawn_auto_start(state.clone());

    let ui_dir = std::env::var("UI_DIR").unwrap_or_else(|_| "ui".to_string());
    let index_file = format!("{}/index.html", ui_dir);

    let app = Router::new()
        .route("/healthz", get(handle_healthz))
        .route("/api/health", get(handle_healthz))
        .route("/api/status", get(handle_get_status))
        .route("/api/config", get(handle_get_config))
        .route("/api/logs", get(handle_get_logs))
        .route("/api/invoke/:cmd", post(handle_invoke))
        .route("/api/logs/stream", get(handle_logs_stream))
        .fallback_service(
            ServeDir::new(&ui_dir).fallback(ServeFile::new(&index_file))
        )
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("═══════════════════════════════════════════════════════════════");
    println!("🚀 Discord Reactor Web Server đang chạy tại http://0.0.0.0:{}", port);
    println!("👉 Truy cập Dashboard qua trình duyệt để điều khiển Bot.");
    println!("═══════════════════════════════════════════════════════════════");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Không thể mở cổng TCP");
    axum::serve(listener, app)
        .await
        .expect("Lỗi chạy máy chủ HTTP");
}
