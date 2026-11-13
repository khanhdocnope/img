use crate::config::ConfigManager;
use crate::discord::client::DiscordClient;
use crate::models::{CategoryConfig, ChannelProgress, EngineStatus};
use chrono::Local;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::time::sleep;

pub type LogCallback = Arc<dyn Fn(String) + Send + Sync + 'static>;

#[derive(Clone)]
pub struct ReactorEngine {
    is_running: Arc<AtomicBool>,
    session_reactions: Arc<AtomicU64>,
    total_channels_scanned: Arc<AtomicUsize>,
    active_account_name: Arc<parking_lot::RwLock<String>>,
}

impl ReactorEngine {
    pub fn new() -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            session_reactions: Arc::new(AtomicU64::new(0)),
            total_channels_scanned: Arc::new(AtomicUsize::new(0)),
            active_account_name: Arc::new(parking_lot::RwLock::new(String::new())),
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub fn get_status(&self, config_mgr: &ConfigManager) -> EngineStatus {
        let is_running = self.is_running();
        let session_reactions = self.session_reactions.load(Ordering::SeqCst);
        let total_channels_scanned = config_mgr.get_progress().len();
        let active_account_name = self.active_account_name.read().clone();

        EngineStatus {
            is_running,
            session_reactions,
            total_channels_scanned,
            active_account_name,
        }
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }

    pub async fn start(
        &self,
        client: DiscordClient,
        account_name: String,
        config_mgr: ConfigManager,
        on_log: LogCallback,
    ) {
        if self.is_running.swap(true, Ordering::SeqCst) {
            // Đang chạy rồi
            return;
        }

        *self.active_account_name.write() = account_name.clone();
        let is_running_clone = self.is_running.clone();
        let session_reactions_clone = self.session_reactions.clone();
        let total_scanned_clone = self.total_channels_scanned.clone();

        tokio::spawn(async move {
            let log = |msg: String| {
                let timestamp = Local::now().format("%H:%M:%S");
                on_log(format!("[{}] {}", timestamp, msg));
            };

            log(format!("Bắt đầu hoạt động cho tài khoản: {}", account_name));

            // Xác thực tài khoản trước
            match client.get_me().await {
                Ok(user) => {
                    log(format!(
                        "Đăng nhập thành công: {} (ID: {})",
                        user.username, user.id
                    ));
                }
                Err(err) => {
                    log(format!("[LỖI] {}", err));
                    is_running_clone.store(false, Ordering::SeqCst);
                    return;
                }
            }

            while is_running_clone.load(Ordering::SeqCst) {
                let config = config_mgr.get_config();
                let categories = config.categories.clone();
                let settings = config.bot_settings.clone();

                if categories.is_empty() {
                    log("Chưa cấu hình Category nào để quét. Tạm nghỉ 30s...".to_string());
                    sleep(Duration::from_secs(30)).await;
                    continue;
                }

                let sem = Arc::new(Semaphore::new(settings.concurrent_channels.max(1)));
                let mut round_reactions = 0u64;

                for cat in categories {
                    if !is_running_clone.load(Ordering::SeqCst) {
                        break;
                    }

                    match Self::process_category(
                        &client,
                        &cat,
                        &config_mgr,
                        sem.clone(),
                        is_running_clone.clone(),
                        session_reactions_clone.clone(),
                        on_log.clone(),
                    )
                    .await
                    {
                        Ok(count) => {
                            round_reactions += count;
                        }
                        Err(e) => {
                            log(format!("[!] Lỗi xử lý category {}: {}", cat.id, e));
                        }
                    }
                }

                total_scanned_clone.store(config_mgr.get_progress().len(), Ordering::SeqCst);
                log(format!(
                    "Hoàn thành lượt quét! Đã thả {} reactions trong lượt này. Nghỉ 10s trước lượt mới...",
                    round_reactions
                ));

                // Cho phép ngắt nghỉ ngay nếu người dùng bấm dừng
                for _ in 0..10 {
                    if !is_running_clone.load(Ordering::SeqCst) {
                        break;
                    }
                    sleep(Duration::from_secs(1)).await;
                }
            }

            log("Bot đã dừng an toàn.".to_string());
        });
    }

    async fn process_category(
        client: &DiscordClient,
        cat: &CategoryConfig,
        config_mgr: &ConfigManager,
        sem: Arc<Semaphore>,
        is_running: Arc<AtomicBool>,
        session_reactions: Arc<AtomicU64>,
        on_log: LogCallback,
    ) -> Result<u64, String> {
        let cat_info = client.get_channel(cat.id).await?;
        let guild_id = cat_info
            .guild_id
            .ok_or_else(|| format!("Category {} không có guild_id", cat.id))?;

        let all_channels = client.get_guild_channels(&guild_id).await?;
        let cat_id_str = cat.id.to_string();

        let target_channels: Vec<_> = all_channels
            .into_iter()
            .filter(|ch| {
                ch.parent_id.as_deref() == Some(&cat_id_str)
                    && ch.channel_type == 0 // GUILD_TEXT
                    && !cat.excluded_channels.contains(&ch.id.parse::<u64>().unwrap_or_default())
            })
            .collect();

        let count = target_channels.len();
        let cat_name = cat_info.name.unwrap_or_else(|| cat.name.clone());
        let log_prefix = format!("[{}]", cat_name);

        {
            let timestamp = Local::now().format("%H:%M:%S");
            on_log(format!(
                "[{}] {} Xử lý {} kênh văn bản...",
                timestamp, log_prefix, count
            ));
        }

        let mut tasks = Vec::new();
        for ch in target_channels {
            if !is_running.load(Ordering::SeqCst) {
                break;
            }

            let client_c = client.clone();
            let config_mgr_c = config_mgr.clone();
            let sem_c = sem.clone();
            let is_running_c = is_running.clone();
            let session_reactions_c = session_reactions.clone();
            let on_log_c = on_log.clone();

            tasks.push(tokio::spawn(async move {
                let _permit = sem_c.acquire().await.unwrap();
                if !is_running_c.load(Ordering::SeqCst) {
                    return 0;
                }
                Self::process_channel(
                    &client_c,
                    &ch.id,
                    ch.name.as_deref().unwrap_or("channel"),
                    &config_mgr_c,
                    is_running_c,
                    session_reactions_c,
                    on_log_c,
                )
                .await
            }));
        }

        let mut total_cat_reactions = 0;
        for task in tasks {
            if let Ok(count) = task.await {
                total_cat_reactions += count;
            }
        }

        Ok(total_cat_reactions)
    }

    async fn process_channel(
        client: &DiscordClient,
        channel_id_str: &str,
        channel_name: &str,
        config_mgr: &ConfigManager,
        is_running: Arc<AtomicBool>,
        session_reactions: Arc<AtomicU64>,
        on_log: LogCallback,
    ) -> u64 {
        let channel_id: u64 = match channel_id_str.parse() {
            Ok(id) => id,
            Err(_) => return 0,
        };

        let progress = config_mgr.get_progress();
        let ch_progress = progress.get(channel_id_str).cloned().unwrap_or_default();
        let last_id = ch_progress.last_reacted_id.and_then(|s| s.parse::<u64>().ok());
        let mut total_reacted = ch_progress.total_reacted;

        let config = config_mgr.get_config();
        let settings = config.bot_settings;
        let limit = if settings.full_scan {
            100
        } else {
            settings.max_messages_per_channel.unwrap_or(100)
        };

        let after = if settings.full_scan { None } else { last_id };

        let messages = match client.get_messages(channel_id, limit, after).await {
            Ok(msgs) => msgs,
            Err(err) => {
                let timestamp = Local::now().format("%H:%M:%S");
                on_log(format!("[{}] #{}: {}", timestamp, channel_name, err));
                return 0;
            }
        };

        let mut new_reacted = 0u64;
        let mut last_message_id = None;

        for msg in messages {
            if !is_running.load(Ordering::SeqCst) {
                break;
            }

            if settings.skip_bot_messages && msg.author.bot {
                continue;
            }
            if msg.reactions.is_empty() {
                continue;
            }

            for r in msg.reactions {
                if !is_running.load(Ordering::SeqCst) {
                    break;
                }
                if r.me {
                    // Đã react rồi, bỏ qua
                    continue;
                }

                let emoji_display = r.emoji.to_display_string();
                if !Self::is_emoji_allowed(&emoji_display, &config.emoji_whitelist, &config.emoji_blacklist) {
                    continue;
                }

                let param = match r.emoji.to_reaction_param() {
                    Some(p) => p,
                    None => continue,
                };

                match client.add_reaction(channel_id, &msg.id, &param).await {
                    Ok(true) => {
                        new_reacted += 1;
                        total_reacted += 1;
                        session_reactions.fetch_add(1, Ordering::SeqCst);
                        last_message_id = Some(msg.id.clone());

                        let timestamp = Local::now().format("%H:%M:%S");
                        on_log(format!(
                            "[{}] [+{}] #{}: tin nhắn {}",
                            timestamp, emoji_display, channel_name, msg.id
                        ));
                    }
                    Ok(false) => {
                        // Forbidden
                    }
                    Err(err) => {
                        let timestamp = Local::now().format("%H:%M:%S");
                        on_log(format!("[{}] [!] Lỗi react tin nhắn {}: {}", timestamp, msg.id, err));
                    }
                }

                let delay_secs = settings.delay.max(0.5);
                sleep(Duration::from_secs_f64(delay_secs)).await;
            }
        }

        if let Some(last_id_str) = last_message_id {
            let mut p_map = config_mgr.get_progress();
            p_map.insert(
                channel_id_str.to_string(),
                ChannelProgress {
                    name: channel_name.to_string(),
                    last_reacted_id: Some(last_id_str),
                    total_reacted,
                },
            );
            let _ = config_mgr.save_progress(p_map);
        }

        new_reacted
    }

    fn is_emoji_allowed(emoji: &str, whitelist: &[String], blacklist: &[String]) -> bool {
        if !whitelist.is_empty() && !whitelist.iter().any(|w| w.trim() == emoji) {
            return false;
        }
        if blacklist.iter().any(|b| b.trim() == emoji) {
            return false;
        }
        true
    }
}
