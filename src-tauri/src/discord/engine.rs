use crate::config::ConfigManager;
use crate::discord::client::{DiscordClient, SendOutcome};
use crate::models::{CategoryConfig, ChannelProgress, EngineStatus};
use chrono::Local;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
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
            total_reactions: config_mgr.get_total_reactions(),
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

            // Luồng gửi emoji định kỳ chạy song song với luồng react
            tokio::spawn(Self::run_emoji_sender(
                client.clone(),
                config_mgr.clone(),
                is_running_clone.clone(),
                on_log.clone(),
            ));

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

            config_mgr.save_stats();
            log("Bot đã dừng an toàn.".to_string());
        });
    }


    /// Bộ sinh số ngẫu nhiên xorshift64 (không cần thêm thư viện ngoài)
    fn next_rand(state: &mut u64) -> u64 {
        let mut x = *state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *state = x;
        x
    }

    /// Ngủ nhưng thoát sớm nếu người dùng bấm dừng
    async fn sleep_while_running(secs: f64, flag: &AtomicBool) {
        let mut left = secs;
        while left > 0.0 && flag.load(Ordering::SeqCst) {
            let step = left.min(1.0);
            sleep(Duration::from_secs_f64(step)).await;
            left -= step;
        }
    }

    /// Tự động gửi ngẫu nhiên (không lặp trong 10 tin gần nhất) emoji của server vào một kênh cố định.
    /// Đọc lại cấu hình ở mỗi vòng nên bật/tắt hoặc đổi kênh có hiệu lực ngay.
    async fn run_emoji_sender(
        client: DiscordClient,
        config_mgr: ConfigManager,
        is_running: Arc<AtomicBool>,
        on_log: LogCallback,
    ) {
        let log = |msg: String| {
            let timestamp = Local::now().format("%H:%M:%S");
            on_log(format!("[{}] {}", timestamp, msg));
        };

        let mut cached_channel: Option<u64> = None;
        let mut cached_animated = false;
        let mut loaded_at: Option<Instant> = None;
        let mut emojis: Vec<String> = Vec::new();
        let mut sent_total: u64 = 0;
        // Các emoji đã gửi gần nhất (mới nhất ở đầu) để tránh lặp trong 10 tin gần nhất
        const RECENT_LIMIT: usize = 10;
        let mut recent: VecDeque<String> = VecDeque::new();
        let mut my_id: Option<String> = None;
        let mut rng_state: u64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15)
            | 1;

        while is_running.load(Ordering::SeqCst) {
            let cfg = config_mgr.get_config().emoji_sender;
            let channel_id = match (cfg.enabled, cfg.channel_id.trim().parse::<u64>().ok()) {
                (true, Some(id)) => id,
                _ => {
                    Self::sleep_while_running(3.0, &is_running).await;
                    continue;
                }
            };

            // Tải (lại) danh sách emoji khi đổi kênh, đổi tuỳ chọn động, hoặc quá hạn
            let ttl = if emojis.is_empty() { 60 } else { 600 };
            let need_reload = cached_channel != Some(channel_id)
                || cached_animated != cfg.allow_animated
                || loaded_at.map_or(true, |t| t.elapsed().as_secs() > ttl);

            if need_reload {
                let guild_id = match client.get_channel(channel_id).await {
                    Ok(ch) => match ch.guild_id {
                        Some(g) => g,
                        None => {
                            log("[Emoji] Kênh này không thuộc server nào, không có emoji để gửi.".to_string());
                            Self::sleep_while_running(30.0, &is_running).await;
                            continue;
                        }
                    },
                    Err(e) => {
                        log(format!("[Emoji] {}", e));
                        Self::sleep_while_running(15.0, &is_running).await;
                        continue;
                    }
                };

                match client.get_guild_emojis(&guild_id).await {
                    Ok(list) => {
                        emojis = list
                            .into_iter()
                            .filter(|e| e.available && (cfg.allow_animated || !e.animated))
                            .filter_map(|e| {
                                let name = e.name?;
                                Some(if e.animated {
                                    format!("<a:{}:{}>", name, e.id)
                                } else {
                                    format!("<:{}:{}>", name, e.id)
                                })
                            })
                            .collect();
                        cached_channel = Some(channel_id);
                        cached_animated = cfg.allow_animated;
                        loaded_at = Some(Instant::now());
                        // Nạp lại 10 tin nhắn gần nhất của chính mình trong kênh (kể cả trước khi mở app)
                        if my_id.is_none() {
                            if let Ok(me) = client.get_me().await {
                                my_id = Some(me.id);
                            }
                        }
                        if let Some(uid) = &my_id {
                            if let Ok(msgs) = client.get_messages(channel_id, 50, None).await {
                                recent = msgs
                                    .into_iter()
                                    .filter(|m| &m.author.id == uid)
                                    .take(RECENT_LIMIT)
                                    .map(|m| m.content.trim().to_string())
                                    .collect();
                            }
                        }
                        log(format!(
                            "[Emoji] Đã tải {} emoji {} của server.",
                            emojis.len(),
                            if cfg.allow_animated { "(gồm cả động)" } else { "tĩnh" }
                        ));
                    }
                    Err(e) => {
                        log(format!("[Emoji] {}", e));
                        Self::sleep_while_running(15.0, &is_running).await;
                        continue;
                    }
                }
            }

            if emojis.is_empty() {
                log("[Emoji] Server không có emoji phù hợp. Thử lại sau 60s.".to_string());
                Self::sleep_while_running(60.0, &is_running).await;
                continue;
            }

            // Chọn ngẫu nhiên, loại các emoji đã gửi trong những tin gần nhất.
            // Nếu server có ít emoji hơn 10 thì thu hẹp cửa sổ để vẫn còn lựa chọn.
            let window = RECENT_LIMIT.min(emojis.len().saturating_sub(1));
            let candidates: Vec<&String> = emojis
                .iter()
                .filter(|e| !recent.iter().take(window).any(|r| r == *e))
                .collect();
            let pool: Vec<&String> = if candidates.is_empty() { emojis.iter().collect() } else { candidates };
            let emoji = pool[(Self::next_rand(&mut rng_state) % pool.len() as u64) as usize].clone();

            match client.send_message(channel_id, &emoji).await {
                Ok(SendOutcome::Sent) => {
                    recent.push_front(emoji.clone());
                    recent.truncate(RECENT_LIMIT);
                    sent_total += 1;
                    log(format!("[Emoji] Đã gửi {} (tổng: {})", emoji, sent_total));
                    Self::sleep_while_running(cfg.interval_secs.max(1) as f64, &is_running).await;
                }
                Ok(SendOutcome::RateLimited(secs)) => {
                    log(format!("[Emoji] Kênh đang giới hạn tốc độ, chờ {:.0}s...", secs + 0.5));
                    Self::sleep_while_running(secs + 0.5, &is_running).await;
                }
                Ok(SendOutcome::Forbidden) => {
                    log("[Emoji] [!] Không có quyền gửi tin nhắn vào kênh này. Thử lại sau 60s.".to_string());
                    Self::sleep_while_running(60.0, &is_running).await;
                }
                Err(e) => {
                    log(format!("[Emoji] [!] {}", e));
                    let wait = if e.starts_with("NET:") { 10.0 } else { 30.0 };
                    Self::sleep_while_running(wait, &is_running).await;
                }
            }
        }
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
        // Nếu có bất kỳ reaction nào lỗi thì KHÔNG đẩy tiến trình lên,
        // để lượt quét sau quét lại và không bỏ sót tin nhắn.
        let mut had_error = false;

        'msgs: for msg in messages {
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
                        config_mgr.count_reaction();
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
                        had_error = true;
                        let timestamp = Local::now().format("%H:%M:%S");
                        on_log(format!("[{}] [!] Lỗi react tin nhắn {}: {}", timestamp, msg.id, err));
                        if err.starts_with("NET:") {
                            // Mất mạng: dừng kênh này, đợi mạng hồi phục rồi quét lại ở lượt sau
                            sleep(Duration::from_secs(10)).await;
                            break 'msgs;
                        }
                    }
                }

                let min_delay = settings.delay.max(0.5);
                let delay_secs = match settings.delay_max {
                    Some(max_d) if max_d > min_delay => {
                        fastrand::f64() * (max_d - min_delay) + min_delay
                    }
                    _ => min_delay,
                };
                sleep(Duration::from_secs_f64(delay_secs)).await;
            }
        }

        if let (false, Some(last_id_str)) = (had_error, last_message_id) {
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

        if new_reacted > 0 {
            config_mgr.save_stats();
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
