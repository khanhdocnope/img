use crate::config::ConfigManager;
use crate::discord::client::{DiscordClient, InvalidMeter, SendOutcome};
use crate::models::{
    Account, BotSettings, CategoryConfig, ChannelProgress, Config, DiscordMessage, EngineStatus,
    MessageComponent, WorkerStatus,
};
use chrono::Local;
use parking_lot::{Mutex, RwLock};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio::time::sleep;

/// Chuyển đổi ký tự tiếng Việt có dấu thành không dấu để so khớp từ khóa siêu chính xác
fn remove_vietnamese_accents(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let repl = match c {
            'á' | 'à' | 'ả' | 'ã' | 'ạ' | 'ă' | 'ắ' | 'ằ' | 'ẳ' | 'ẵ' | 'ặ' | 'â' | 'ấ' | 'ầ' | 'ẩ' | 'ẫ' | 'ậ' => 'a',
            'é' | 'è' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ế' | 'ề' | 'ể' | 'ễ' | 'ệ' => 'e',
            'í' | 'ì' | 'ỉ' | 'ĩ' | 'ị' => 'i',
            'ó' | 'ò' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ố' | 'ồ' | 'ổ' | 'ỗ' | 'ộ' | 'ơ' | 'ớ' | 'ờ' | 'ở' | 'ỡ' | 'ợ' => 'o',
            'ú' | 'ù' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ứ' | 'ừ' | 'ử' | 'ữ' | 'ự' => 'u',
            'ý' | 'ỳ' | 'ỷ' | 'ỹ' | 'ỵ' => 'y',
            'đ' => 'd',
            'Á' | 'À' | 'Ả' | 'Ã' | 'Ạ' | 'Ă' | 'Ắ' | 'Ằ' | 'Ẳ' | 'Ẵ' | 'Ặ' | 'Â' | 'Ấ' | 'Ầ' | 'Ẩ' | 'Ẫ' | 'Ậ' => 'a',
            'É' | 'È' | 'Ẻ' | 'Ẽ' | 'Ẹ' | 'Ê' | 'Ế' | 'Ề' | 'Ể' | 'Ễ' | 'Ệ' => 'e',
            'Í' | 'Ì' | 'Ỉ' | 'Ĩ' | 'Ị' => 'i',
            'Ó' | 'Ò' | 'Ỏ' | 'Õ' | 'Ọ' | 'Ô' | 'Ố' | 'Ồ' | 'Ổ' | 'Ỗ' | 'Ộ' | 'Ơ' | 'Ớ' | 'Ờ' | 'Ở' | 'Ỡ' | 'Ợ' => 'o',
            'Ú' | 'Ù' | 'Ủ' | 'Ũ' | 'Ụ' | 'Ư' | 'Ứ' | 'Ừ' | 'Ử' | 'Ữ' | 'Ự' => 'u',
            'Ý' | 'Ỳ' | 'Ỷ' | 'Ỹ' | 'Ỵ' => 'y',
            'Đ' => 'd',
            other => other,
        };
        out.push(repl);
    }
    out
}

/// Chuẩn hóa chuỗi: chuyển chữ thường, bỏ biến thể Unicode FE0F, khoảng trắng thừa, bỏ dấu tiếng Việt
fn norm_text(s: &str) -> String {
    let cleaned = s.replace('\u{fe0f}', "").replace('\u{00a0}', " ");
    let unaccented = remove_vietnamese_accents(&cleaned);
    unaccented.trim().to_lowercase()
}

#[derive(Debug, Clone)]
struct ExtractedButton {
    custom_id: String,
    label: String,
    emoji_name: String,
    disabled: bool,
}

/// Số phản hồi lỗi (401/403/429) trong 10 phút mà tại đó mọi tài khoản tạm nghỉ để tránh bị chặn IP
const INVALID_LIMIT: usize = 120;

pub type LogCallback = Arc<dyn Fn(String) + Send + Sync + 'static>;

/// Số ngẫu nhiên trong [0, 1), an toàn khi gọi từ nhiều luồng (splitmix64 trộn thêm đồng hồ)
fn rand_unit() -> f64 {
    static STATE: AtomicU64 = AtomicU64::new(0x853C_49E6_748F_EA9B);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let step = 0x9E37_79B9_7F4A_7C15u64;
    let mut z = STATE.fetch_add(step, Ordering::Relaxed).wrapping_add(step) ^ nanos;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// Số ngẫu nhiên trong [min, max]
fn rand_between(min: f64, max: f64) -> f64 {
    if max <= min {
        min
    } else {
        min + rand_unit() * (max - min)
    }
}

/// Bối cảnh riêng của một tài khoản đang chạy (mỗi tài khoản có client, nhịp và bộ bảo vệ riêng)
struct WorkerCtx {
    client: DiscordClient,
    account_id: String,
    account_name: String,
    /// Tiền tố log khi chạy nhiều tài khoản, ví dụ "[Acc chính] "
    tag: String,
    /// Chạy nhiều tài khoản cùng lúc
    multi: bool,
    /// Tài khoản đầu tiên dùng khóa tiến trình cũ (chỉ ID kênh) để giữ dữ liệu của bản trước
    legacy_keys: bool,
    config_mgr: ConfigManager,
    /// Cờ chạy riêng của tài khoản này (tắt khi bấm Dừng hoặc khi token bị từ chối)
    run: Arc<AtomicBool>,
    /// Tổng reaction của phiên (mọi tài khoản)
    session_reactions: Arc<AtomicU64>,
    /// Reaction của riêng tài khoản này
    my_reactions: Arc<AtomicU64>,
    on_log: LogCallback,
    meter: Arc<InvalidMeter>,
    /// Hệ số tốc độ ngẫu nhiên riêng của tài khoản (0.9–1.3) để hai nhịp không giống nhau
    speed: f64,
    strikes: AtomicU32,
    cooldown_until_ms: AtomicU64,
    /// Các kênh không có quyền react: bỏ qua một thời gian thay vì gọi lại mỗi lượt
    blocked: Mutex<HashMap<u64, Instant>>,
    /// Cache các nút đã click (dạng "message_id:custom_id" -> Instant) với cooldown 8s để có thể đánh lại khi sang hiệp/hồi sinh
    clicked_buttons: Mutex<HashMap<String, Instant>>,
}

impl WorkerCtx {
    fn alive(&self) -> bool {
        self.run.load(Ordering::SeqCst)
    }

    fn log(&self, msg: String) {
        let timestamp = Local::now().format("%H:%M:%S");
        (self.on_log)(format!("[{}] {}{}", timestamp, self.tag, msg));
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn cooldown_left(&self) -> f64 {
        let until = self.cooldown_until_ms.load(Ordering::SeqCst);
        let now = Self::now_ms();
        if until > now {
            (until - now) as f64 / 1000.0
        } else {
            0.0
        }
    }

    /// Ghi nhận một đợt rate limit kéo dài, trả về số giây phải nghỉ (60s -> 180s -> 600s)
    fn strike(&self) -> u64 {
        let n = self.strikes.fetch_add(1, Ordering::SeqCst) + 1;
        let secs = match n {
            1 => 60,
            2 => 180,
            _ => 600,
        };
        self.cooldown_until_ms
            .store(Self::now_ms() + secs * 1000, Ordering::SeqCst);
        secs
    }

    fn channel_blocked(&self, channel_id: u64) -> bool {
        let mut map = self.blocked.lock();
        match map.get(&channel_id) {
            Some(t) if t.elapsed() < Duration::from_secs(86400) => true,
            Some(_) => {
                map.remove(&channel_id);
                false
            }
            None => false,
        }
    }

    fn block_channel(&self, channel_id: u64) {
        self.blocked.lock().insert(channel_id, Instant::now());
    }

    fn progress_key(&self, channel_id_str: &str) -> String {
        if self.legacy_keys {
            channel_id_str.to_string()
        } else {
            format!("{}:{}", self.account_id, channel_id_str)
        }
    }

    /// Lỗi nghiêm trọng (token bị từ chối): dừng riêng tài khoản này
    fn fatal(&self, reason: &str) {
        self.log(format!("[LỖI] {}", reason));
        self.run.store(false, Ordering::SeqCst);
    }
}

#[derive(Clone)]
struct WorkerHandle {
    account_id: String,
    name: String,
    run: Arc<AtomicBool>,
    reactions: Arc<AtomicU64>,
    /// (trạng thái, ghi chú)
    state: Arc<RwLock<(String, String)>>,
    emoji_sent: Arc<AtomicU64>,
    emoji_note: Arc<RwLock<String>>,
}

pub struct ReactorEngine {
    is_running: Arc<AtomicBool>,
    session_reactions: Arc<AtomicU64>,
    active_workers: Arc<AtomicUsize>,
    workers: Arc<RwLock<Vec<WorkerHandle>>>,
    meter: Arc<InvalidMeter>,
}

impl ReactorEngine {
    pub fn new() -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            session_reactions: Arc::new(AtomicU64::new(0)),
            active_workers: Arc::new(AtomicUsize::new(0)),
            workers: Arc::new(RwLock::new(Vec::new())),
            meter: Arc::new(InvalidMeter::default()),
        }
    }

    /// Bộ đếm phản hồi lỗi dùng chung, gắn vào client của mọi tài khoản
    pub fn meter(&self) -> Arc<InvalidMeter> {
        self.meter.clone()
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub fn get_status(&self, config_mgr: &ConfigManager) -> EngineStatus {
        let is_running = self.is_running();
        let session_reactions = self.session_reactions.load(Ordering::SeqCst);

        // Cùng một kênh có thể có khóa tiến trình của nhiều tài khoản: đếm theo ID kênh
        let total_channels_scanned = config_mgr
            .get_progress()
            .keys()
            .map(|k| k.rsplit(':').next().unwrap_or(k).to_string())
            .collect::<HashSet<_>>()
            .len();

        let workers: Vec<WorkerStatus> = self
            .workers
            .read()
            .iter()
            .map(|w| {
                let (state, note) = w.state.read().clone();
                WorkerStatus {
                    account_id: w.account_id.clone(),
                    name: w.name.clone(),
                    state,
                    note,
                    reactions: w.reactions.load(Ordering::SeqCst),
                    emoji_sent: w.emoji_sent.load(Ordering::SeqCst),
                    emoji_note: w.emoji_note.read().clone(),
                }
            })
            .collect();

        let mut names: Vec<String> = workers.iter().map(|w| w.name.clone()).collect();
        if names.is_empty() {
            names = config_mgr
                .get_running_accounts()
                .into_iter()
                .map(|a| a.name)
                .collect();
        }

        let progress_list: Vec<ChannelProgress> = config_mgr.get_progress().into_values().collect();

        EngineStatus {
            is_running,
            session_reactions,
            total_reactions: config_mgr.get_total_reactions(),
            total_channels_scanned,
            active_account_name: names.join(" + "),
            workers,
            progress_list,
        }
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
        for w in self.workers.read().iter() {
            w.run.store(false, Ordering::SeqCst);
        }
    }

    /// Chạy mỗi tài khoản một worker độc lập. Tài khoản thứ hai bắt đầu trễ 20–45s.
    pub async fn start(
        &self,
        accounts: Vec<(Account, DiscordClient)>,
        config_mgr: ConfigManager,
        on_log: LogCallback,
    ) {
        if accounts.is_empty() {
            return;
        }
        if self.is_running.swap(true, Ordering::SeqCst) {
            // Đang chạy rồi
            return;
        }

        let multi = accounts.len() > 1;
        self.active_workers.store(accounts.len(), Ordering::SeqCst);

        let mut handles: Vec<WorkerHandle> = Vec::new();
        for (idx, (account, client)) in accounts.into_iter().enumerate() {
            let run = Arc::new(AtomicBool::new(true));
            let reactions = Arc::new(AtomicU64::new(0));
            let state = Arc::new(RwLock::new(("starting".to_string(), String::new())));
            let emoji_sent = Arc::new(AtomicU64::new(0));
            let emoji_note = Arc::new(RwLock::new("Chưa gửi".to_string()));

            handles.push(WorkerHandle {
                account_id: account.id.clone(),
                name: account.name.clone(),
                run: run.clone(),
                reactions: reactions.clone(),
                state: state.clone(),
                emoji_sent: emoji_sent.clone(),
                emoji_note: emoji_note.clone(),
            });

            let ctx = Arc::new(WorkerCtx {
                client,
                account_id: account.id.clone(),
                account_name: account.name.clone(),
                tag: if multi { format!("[{}] ", account.name) } else { String::new() },
                multi,
                legacy_keys: idx == 0,
                config_mgr: config_mgr.clone(),
                run,
                session_reactions: self.session_reactions.clone(),
                my_reactions: reactions,
                on_log: on_log.clone(),
                meter: self.meter.clone(),
                speed: 1.0,
                strikes: AtomicU32::new(0),
                cooldown_until_ms: AtomicU64::new(0),
                blocked: Mutex::new(HashMap::new()),
                clicked_buttons: Mutex::new(HashMap::new()),
            });

            let start_delay = if idx == 0 { 0.0 } else { rand_between(20.0, 45.0) };
            let is_running = self.is_running.clone();
            let active = self.active_workers.clone();
            let es_sent = emoji_sent.clone();
            let es_note = emoji_note.clone();

            tokio::spawn(async move {
                Self::run_worker(ctx, idx, start_delay, state, es_sent, es_note).await;
                // Worker cuối cùng thoát thì đánh dấu bot đã dừng
                if active.fetch_sub(1, Ordering::SeqCst) == 1 {
                    is_running.store(false, Ordering::SeqCst);
                }
            });
        }
        *self.workers.write() = handles;
    }

    async fn run_worker(
        ctx: Arc<WorkerCtx>,
        idx: usize,
        start_delay: f64,
        state: Arc<RwLock<(String, String)>>,
        emoji_sent: Arc<AtomicU64>,
        emoji_note: Arc<RwLock<String>>,
    ) {
        let set = |s: &str, note: &str| {
            *state.write() = (s.to_string(), note.to_string());
        };

        if start_delay > 0.0 {
            set("starting", &format!("Bắt đầu sau {:.0}s", start_delay));
            ctx.log(format!(
                "Sẽ bắt đầu sau {:.0}s để không trùng nhịp với tài khoản còn lại.",
                start_delay
            ));
            Self::sleep_while_running(start_delay, &ctx.run).await;
            if !ctx.alive() {
                set("stopped", "");
                return;
            }
        }

        ctx.log(format!("Bắt đầu hoạt động cho tài khoản: {}", ctx.account_name));

        // Xác thực tài khoản trước
        match ctx.client.get_me().await {
            Ok(user) => ctx.log(format!("Đăng nhập thành công: {} (ID: {})", user.username, user.id)),
            Err(err) => {
                ctx.log(format!("[LỖI] {}", err));
                set("error", "Đăng nhập thất bại");
                ctx.run.store(false, Ordering::SeqCst);
                return;
            }
        }

        // Mỗi tài khoản đang chạy đều gửi emoji định kỳ vào kênh được cài đặt
        // Tài khoản thứ 2 trở đi sẽ bắt đầu trễ 15s để so le thời gian gửi giữa các tài khoản
        let sender_delay = if idx == 0 { 0.0 } else { 15.0 };
        tokio::spawn(Self::run_emoji_sender(
            ctx.account_name.clone(),
            ctx.client.clone(),
            ctx.config_mgr.clone(),
            ctx.run.clone(),
            ctx.on_log.clone(),
            sender_delay,
            emoji_sent,
            emoji_note,
        ));

        // Luồng chuyên canh kênh đánh Boss riêng (chạy song song real-time)
        let ctx_boss = ctx.clone();
        tokio::spawn(async move {
            Self::run_boss_watcher(ctx_boss).await;
        });

        while ctx.alive() {
            // Tạm nghỉ khi bị rate limit kéo dài
            let cool = ctx.cooldown_left();
            if cool > 0.0 {
                set("cooldown", &format!("Nghỉ {:.0}s do bị giới hạn tốc độ", cool));
                Self::sleep_while_running(cool.min(30.0), &ctx.run).await;
                continue;
            }
            // Quá nhiều phản hồi lỗi gần đây (cả hai tài khoản): nghỉ để tránh bị chặn IP
            if ctx.meter.recent() >= INVALID_LIMIT {
                ctx.log("[!] Có quá nhiều phản hồi lỗi (401/403/429) gần đây. Tạm nghỉ 10 phút để an toàn.".to_string());
                set("cooldown", "Nghỉ 10 phút (quá nhiều phản hồi lỗi)");
                Self::sleep_while_running(600.0, &ctx.run).await;
                continue;
            }

            set("running", "");
            let config = ctx.config_mgr.get_config();
            let categories = config.categories.clone();
            let settings = config.bot_settings.clone();

            if categories.is_empty() {
                ctx.log("Chưa cấu hình Category nào để quét. Tạm nghỉ 30s...".to_string());
                Self::sleep_while_running(30.0, &ctx.run).await;
                continue;
            }

            // Chạy nhiều tài khoản thì mỗi tài khoản dùng một nửa số kênh song song
            let mut conc = settings.concurrent_channels.max(1);
            if ctx.multi {
                conc = (conc + 1) / 2;
            }
            let sem = Arc::new(Semaphore::new(conc.max(1)));
            let strikes_before = ctx.strikes.load(Ordering::SeqCst);
            let mut round_reactions = 0u64;

            for cat in categories {
                if !ctx.alive() || ctx.cooldown_left() > 0.0 {
                    break;
                }

                match Self::process_category(&ctx, &cat, sem.clone()).await {
                    Ok(count) => round_reactions += count,
                    Err(e) => {
                        if e.starts_with("AUTH:") {
                            ctx.fatal("Token bị Discord từ chối (401). Tài khoản này đã dừng, hãy kiểm tra lại token.");
                            set("error", "Token bị từ chối (401)");
                        } else if e.starts_with("RATE:") {
                            let secs = ctx.strike();
                            ctx.log(format!("[!] Bị giới hạn tốc độ khi đọc dữ liệu, nghỉ {}s.", secs));
                        } else {
                            ctx.log(format!("[!] Lỗi xử lý category {}: {}", cat.id, e));
                        }
                    }
                }
            }

            // Một lượt quét sạch (không bị rate limit) thì xóa dấu vết vi phạm
            if ctx.alive() && ctx.strikes.load(Ordering::SeqCst) == strikes_before {
                ctx.strikes.store(0, Ordering::SeqCst);
            }

            if ctx.alive() {
                ctx.log(format!(
                    "Hoàn thành lượt quét! Đã thả {} reactions trong lượt này. Nghỉ vài giây trước lượt mới...",
                    round_reactions
                ));
                // Nghỉ ngẫu nhiên 5–10s (nhân hệ số riêng), ngắt ngay nếu người dùng bấm dừng
                Self::sleep_while_running(rand_between(5.0, 10.0) * ctx.speed, &ctx.run).await;
            }
        }

        ctx.config_mgr.save_stats();
        if state.read().0 != "error" {
            set("stopped", "");
        }
        ctx.log("Đã dừng an toàn.".to_string());
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

    /// Luồng chuyên canh kênh đánh Boss độc lập (real-time, không bị ảnh hưởng bởi việc quét reaction)
    async fn run_boss_watcher(ctx: Arc<WorkerCtx>) {
        let mut cached_channel_id: Option<u64> = None;
        let mut cached_guild_id: Option<String> = None;
        let mut cached_channel_name = String::new();
        let mut logged_empty = false;

        while ctx.alive() {
            let config = ctx.config_mgr.get_config();
            let settings = config.bot_settings;

            if !settings.auto_button_enabled {
                Self::sleep_while_running(3.0, &ctx.run).await;
                continue;
            }

            let ch_str = settings.button_channel_id.trim();
            if ch_str.is_empty() {
                if !logged_empty {
                    ctx.log("[⚔️ Boss Watcher] ℹ️ Chưa cài đặt 'ID Kênh đánh Boss' (đang để trống) -> Tool chỉ tìm boss trong Danh mục đang quét.".to_string());
                    logged_empty = true;
                }
                Self::sleep_while_running(3.0, &ctx.run).await;
                continue;
            }
            logged_empty = false;

            let ch_id: u64 = match ch_str.parse() {
                Ok(id) => id,
                Err(_) => {
                    Self::sleep_while_running(5.0, &ctx.run).await;
                    continue;
                }
            };

            // Nếu đổi kênh hoặc chưa nạp thông tin kênh
            if cached_channel_id != Some(ch_id) {
                match ctx.client.get_channel(ch_id).await {
                    Ok(ch) => {
                        cached_guild_id = ch.guild_id.clone();
                        cached_channel_name = ch.name.unwrap_or_else(|| ch_id.to_string());
                        cached_channel_id = Some(ch_id);
                        ctx.log(format!(
                            "[⚔️ Boss Watcher] 🎯 Bắt đầu canh kênh Boss riêng: #{} ({})",
                            cached_channel_name, ch_id
                        ));
                    }
                    Err(e) => {
                        ctx.log(format!("[⚔️ Boss Watcher] Không thể đọc thông tin kênh boss {}: {}", ch_id, e));
                        Self::sleep_while_running(15.0, &ctx.run).await;
                        continue;
                    }
                }
            }

            let gid = match &cached_guild_id {
                Some(g) => g.clone(),
                None => {
                    Self::sleep_while_running(10.0, &ctx.run).await;
                    continue;
                }
            };

            // Lấy 15 tin nhắn mới nhất trên cùng của kênh (tránh trôi tin khi có người chat)
            match ctx.client.get_messages_before(ch_id, 15, None).await {
                Ok(msgs) => {
                    Self::check_and_click_buttons(&ctx, &gid, ch_id, &cached_channel_name, &msgs, &settings).await;
                }
                Err(err) => {
                    if err.contains("403") {
                        ctx.log(format!("[⚔️ Boss Watcher] 403 Forbidden tại kênh boss #{}.", cached_channel_name));
                        Self::sleep_while_running(30.0, &ctx.run).await;
                        continue;
                    }
                }
            }

            // Canh mỗi 2 - 3.5 giây
            Self::sleep_while_running(rand_between(2.0, 3.5), &ctx.run).await;
        }
    }

    /// Tự động gửi ngẫu nhiên (không lặp trong 10 tin gần nhất) emoji của server vào một kênh cố định.
    /// Đọc lại cấu hình ở mỗi vòng nên bật/tắt hoặc đổi kênh có hiệu lực ngay.
    async fn run_emoji_sender(
        account_name: String,
        client: DiscordClient,
        config_mgr: ConfigManager,
        is_running: Arc<AtomicBool>,
        on_log: LogCallback,
        initial_delay: f64,
        emoji_sent: Arc<AtomicU64>,
        emoji_note: Arc<RwLock<String>>,
    ) {
        let tag = format!("[Emoji: {}]", account_name);
        let log = {
            let tag = tag.clone();
            move |msg: String| {
                let timestamp = Local::now().format("%H:%M:%S");
                on_log(format!("[{}] {} {}", timestamp, tag, msg));
            }
        };

        if initial_delay > 0.0 {
            *emoji_note.write() = format!("Khởi động sau {:.0}s", initial_delay);
            Self::sleep_while_running(initial_delay, &is_running).await;
        }

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
                    *emoji_note.write() = "Chưa bật / chưa cấu hình".to_string();
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
                            log("Kênh này không thuộc server nào, không có emoji để gửi.".to_string());
                            Self::sleep_while_running(30.0, &is_running).await;
                            continue;
                        }
                    },
                    Err(e) => {
                        log(e);
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
                            "Đã tải {} emoji {} của server.",
                            emojis.len(),
                            if cfg.allow_animated { "(gồm cả động)" } else { "tĩnh" }
                        ));
                    }
                    Err(e) => {
                        log(e);
                        Self::sleep_while_running(15.0, &is_running).await;
                        continue;
                    }
                }
            }

            if emojis.is_empty() {
                log("Server không có emoji phù hợp. Thử lại sau 60s.".to_string());
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
                    emoji_sent.fetch_add(1, Ordering::SeqCst);
                    *emoji_note.write() = format!("Đã gửi {}", emoji);
                    log(format!("Đã gửi {} (tổng: {})", emoji, sent_total));
                    // Cộng thêm 0–1,5s ngẫu nhiên để không đúng mép slowmode
                    Self::sleep_while_running(cfg.interval_secs.max(1) as f64 + rand_between(0.0, 1.5), &is_running).await;
                }
                Ok(SendOutcome::RateLimited(secs)) => {
                    *emoji_note.write() = format!("Slowmode {:.0}s", secs + 0.5);
                    log(format!("Kênh đang giới hạn tốc độ, chờ {:.0}s...", secs + 0.5));
                    Self::sleep_while_running(secs + 0.5, &is_running).await;
                }
                Ok(SendOutcome::Forbidden) => {
                    *emoji_note.write() = "Không có quyền gửi".to_string();
                    log("[!] Không có quyền gửi tin nhắn vào kênh này. Thử lại sau 60s.".to_string());
                    Self::sleep_while_running(60.0, &is_running).await;
                }
                Err(e) => {
                    *emoji_note.write() = format!("Lỗi: {}", e);
                    log(format!("[!] {}", e));
                    let wait = if e.starts_with("NET:") { 10.0 } else { 30.0 };
                    Self::sleep_while_running(wait, &is_running).await;
                }
            }
        }
    }

    async fn process_category(
        ctx: &Arc<WorkerCtx>,
        cat: &CategoryConfig,
        sem: Arc<Semaphore>,
    ) -> Result<u64, String> {
        let cat_info = ctx.client.get_channel(cat.id).await?;
        let guild_id = cat_info
            .guild_id
            .ok_or_else(|| format!("Category {} không có guild_id", cat.id))?;

        let all_channels = ctx.client.get_guild_channels(&guild_id).await?;
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
        ctx.log(format!("[{}] Xử lý {} kênh văn bản...", cat_name, count));

        let mut tasks = Vec::new();
        for ch in target_channels {
            if !ctx.alive() {
                break;
            }

            let ctx_c = ctx.clone();
            let sem_c = sem.clone();
            let gid = ch.guild_id.clone().unwrap_or_else(|| guild_id.clone());
            let ch_id = ch.id.clone();
            let ch_name = ch.name.clone().unwrap_or_else(|| "channel".to_string());

            tasks.push(tokio::spawn(async move {
                let _permit = sem_c.acquire().await.unwrap();
                if !ctx_c.alive() {
                    return 0;
                }
                Self::process_channel(&ctx_c, &gid, &ch_id, &ch_name).await
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

    /// Trích xuất toàn bộ các nút bấm từ các components (hỗ trợ ActionRow hoặc lồng nhau)
    fn extract_all_buttons(components: &[MessageComponent]) -> Vec<ExtractedButton> {
        let mut results = Vec::new();
        for comp in components {
            Self::collect_buttons_recursive(comp, &mut results);
        }
        results
    }

    fn collect_buttons_recursive(comp: &MessageComponent, out: &mut Vec<ExtractedButton>) {
        if comp.component_type == 2 || comp.custom_id.is_some() {
            if let Some(ref cid) = comp.custom_id {
                if !cid.is_empty() {
                    out.push(ExtractedButton {
                        custom_id: cid.clone(),
                        label: comp.label.clone().unwrap_or_default(),
                        emoji_name: comp.emoji.as_ref().and_then(|e| e.name.clone()).unwrap_or_default(),
                        disabled: comp.disabled.unwrap_or(false),
                    });
                }
            }
        }
        for sub in &comp.components {
            Self::collect_buttons_recursive(sub, out);
        }
    }

    /// Tự động kiểm tra và bấm nút Boss trên một danh sách tin nhắn
    async fn check_and_click_buttons(
        ctx: &Arc<WorkerCtx>,
        guild_id: &str,
        channel_id: u64,
        channel_name: &str,
        messages: &[DiscordMessage],
        settings: &BotSettings,
    ) {
        if !settings.auto_button_enabled {
            return;
        }

        // Tự động dọn dẹp các nút đã ghi nhận quá 10 phút
        {
            let mut clicked = ctx.clicked_buttons.lock();
            let now = Instant::now();
            clicked.retain(|_, time| now.duration_since(*time) < Duration::from_secs(600));
        }

        for msg in messages {
            if !ctx.alive() {
                break;
            }

            // Chỉ quan tâm tin nhắn có component hoặc tin nhắn từ Bot
            if msg.components.is_empty() && !msg.author.bot {
                continue;
            }

            let buttons = Self::extract_all_buttons(&msg.components);
            if buttons.is_empty() {
                continue;
            }

            for btn in buttons {
                if btn.disabled {
                    continue;
                }
                if btn.custom_id.is_empty() {
                    continue;
                }

                let norm_label = norm_text(&btn.label);
                let norm_emoji = norm_text(&btn.emoji_name);

                let label_keywords: Vec<String> = settings
                    .button_label_keyword
                    .split(',')
                    .map(|s| norm_text(s))
                    .filter(|s| !s.is_empty())
                    .collect();
                let emoji_keywords: Vec<String> = settings
                    .button_emoji_keyword
                    .split(',')
                    .map(|s| norm_text(s))
                    .filter(|s| !s.is_empty())
                    .collect();

                let label_match = (!label_keywords.is_empty() && label_keywords.iter().any(|kw| norm_label.contains(kw) || kw.contains(&norm_label)))
                    || norm_label.contains("danh boss")
                    || norm_label.contains("tan cong")
                    || norm_label.contains("attack");

                let emoji_match = (!emoji_keywords.is_empty() && emoji_keywords.iter().any(|kw| norm_emoji.contains(kw) || kw.contains(&norm_emoji)))
                    || norm_emoji.contains("⚔")
                    || norm_label.contains("⚔");

                if label_match || emoji_match {
                    let click_key = format!("{}:{}", msg.id, btn.custom_id);
                    {
                        let clicked = ctx.clicked_buttons.lock();
                        if let Some(last_time) = clicked.get(&click_key) {
                            if last_time.elapsed() < Duration::from_secs(8) {
                                // Cooldown 8 giây cho cùng 1 nút để tránh spam đúp
                                continue;
                            }
                        }
                    }

                    let app_id = msg.application_id.as_deref().unwrap_or(&msg.author.id);

                    ctx.log(format!(
                        "[⚔️ Đánh Boss] Phát hiện nút \"{}\" (emoji: {}) tại #{}, đang bấm...",
                        if !btn.label.is_empty() { &btn.label } else { "(không chữ)" },
                        if !btn.emoji_name.is_empty() { &btn.emoji_name } else { "none" },
                        channel_name
                    ));

                    match ctx
                        .client
                        .click_button(Some(guild_id), channel_id, &msg.id, app_id, &btn.custom_id)
                        .await
                    {
                        Ok(true) => {
                            ctx.clicked_buttons.lock().insert(click_key, Instant::now());
                            ctx.log(format!(
                                "[⚔️ Đánh Boss] ✅ Đã đánh thành công tại #{} (tin {})",
                                channel_name, msg.id
                            ));
                        }
                        Ok(false) => {}
                        Err(err) => {
                            ctx.log(format!("[⚔️ Đánh Boss] ❌ Lỗi bấm nút: {}", err));
                        }
                    }

                    sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }

    /// Thả reaction cho một lô tin nhắn.
    /// Trả về (số reaction mới, id tin cuối cùng đã react, có lỗi tạm thời không).
    async fn react_messages(
        ctx: &Arc<WorkerCtx>,
        guild_id: &str,
        channel_id: u64,
        channel_name: &str,
        messages: Vec<DiscordMessage>,
        settings: &BotSettings,
        config: &Config,
    ) -> (u64, Option<String>, bool) {
        let mut new_reacted = 0u64;
        let mut last_message_id = None;
        // Chỉ lỗi tạm thời (mất mạng / rate limit kéo dài) mới giữ nguyên tiến trình để quét lại.
        // Lỗi vĩnh viễn (vd. tin đã đủ 20 loại emoji, emoji đã bị xóa) thì bỏ qua,
        // nếu không một tin lỗi sẽ chặn cả kênh.
        let mut had_error = false;

        'msgs: for msg in messages {
            if !ctx.alive() {
                break;
            }

            // 1. Tự động click nút (Đánh Boss ⚔️) trên tin nhắn (kể cả tin nhắn của Bot)
            if settings.auto_button_enabled {
                Self::check_and_click_buttons(ctx, guild_id, channel_id, channel_name, std::slice::from_ref(&msg), settings).await;
            }

            // 2. Thả reaction cho tin nhắn
            if settings.skip_bot_messages && msg.author.bot {
                continue;
            }
            if msg.reactions.is_empty() {
                continue;
            }

            for r in msg.reactions {
                if !ctx.alive() {
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

                match ctx.client.add_reaction(channel_id, &msg.id, &param).await {
                    Ok(true) => {
                        new_reacted += 1;
                        ctx.session_reactions.fetch_add(1, Ordering::SeqCst);
                        ctx.my_reactions.fetch_add(1, Ordering::SeqCst);
                        ctx.config_mgr.count_reaction();
                        last_message_id = Some(msg.id.clone());

                        ctx.log(format!("[+{}] #{}: tin nhắn {}", emoji_display, channel_name, msg.id));
                    }
                    Ok(false) => {
                        // 403: không có quyền react. Bỏ qua kênh này một lúc để không gọi lại liên tục
                        ctx.block_channel(channel_id);
                        ctx.log(format!(
                            "[!] #{}: không có quyền react, bỏ qua kênh này 30 phút.",
                            channel_name
                        ));
                        break 'msgs;
                    }
                    Err(err) => {
                        if err.starts_with("AUTH:") {
                            ctx.fatal("Token bị Discord từ chối (401). Tài khoản này đã dừng, hãy kiểm tra lại token.");
                            had_error = true;
                            break 'msgs;
                        }
                        if err.starts_with("Đã vượt quá") {
                            had_error = true;
                            let secs = ctx.strike();
                            ctx.log(format!("[!] Bị giới hạn tốc độ kéo dài, tạm nghỉ {}s.", secs));
                            break 'msgs;
                        }
                        if err.starts_with("NET:") {
                            had_error = true;
                            ctx.log(format!("[!] Lỗi react tin nhắn {}: {}", msg.id, err));
                            // Mất mạng: dừng kênh này, đợi mạng hồi phục rồi quét lại ở lượt sau
                            sleep(Duration::from_secs(10)).await;
                            break 'msgs;
                        }
                        ctx.log(format!("[!] Lỗi react tin nhắn {}: {}", msg.id, err));
                    }
                }

                // Tick ngẫu nhiên trong [delay, delay_max]
                let lo = settings.delay.max(0.05);
                let hi = if settings.delay_max > lo { settings.delay_max } else { lo };
                let wait_time = if hi > lo { rand_between(lo, hi) } else { lo };
                sleep(Duration::from_secs_f64(wait_time)).await;
            }
        }

        (new_reacted, last_message_id, had_error)
    }

    async fn process_channel(
        ctx: &Arc<WorkerCtx>,
        guild_id: &str,
        channel_id_str: &str,
        channel_name: &str,
    ) -> u64 {
        let channel_id: u64 = match channel_id_str.parse() {
            Ok(id) => id,
            Err(_) => return 0,
        };
        if !ctx.alive() || ctx.cooldown_left() > 0.0 || ctx.channel_blocked(channel_id) {
            return 0;
        }

        let key = ctx.progress_key(channel_id_str);
        let ch_progress = ctx
            .config_mgr
            .get_progress()
            .get(&key)
            .cloned()
            .unwrap_or_default();
        let last_id = ch_progress.last_reacted_id.and_then(|s| s.parse::<u64>().ok());

        let config = ctx.config_mgr.get_config();
        let settings = config.bot_settings.clone();

        let mut total_channel_reacted = 0u64;
        let mut newest_msg_id: Option<String> = None;
        let mut total_messages_scanned = 0u32;
        let max_msgs = settings.max_messages_per_channel;

        // ── 0. ƯU TIÊN SỐ 1: BẮT BOSS TẠI CÁC TIN NHẮN MỚI NHẤT CỦA KÊNH ──
        if settings.auto_button_enabled {
            if let Ok(latest_msgs) = ctx.client.get_messages_before(channel_id, 10, None).await {
                Self::check_and_click_buttons(ctx, guild_id, channel_id, channel_name, &latest_msgs, &settings).await;
            }
        }

        if settings.full_scan {
            // ── QUÉT TOÀN BỘ LỊCH SỬ KÊNH (Duyệt liên tục qua các trang 100 tin, không bị chặn) ──
            // Nếu bật resume_progress và chưa quét xong lịch sử, tiếp tục từ history_cursor đã lưu
            let mut before_cursor: Option<u64> = if settings.resume_progress && !ch_progress.history_done {
                ch_progress.history_cursor.as_deref().and_then(|s| s.parse::<u64>().ok())
            } else {
                None
            };

            loop {
                if !ctx.alive() || ctx.cooldown_left() > 0.0 {
                    break;
                }

                let fetch_limit = if let Some(max) = max_msgs {
                    if total_messages_scanned >= max {
                        break;
                    }
                    (max - total_messages_scanned).min(100)
                } else {
                    100
                };

                let page = match ctx.client.get_messages_before(channel_id, fetch_limit, before_cursor).await {
                    Ok(msgs) => msgs,
                    Err(err) => {
                        if err.contains("403") || err.contains("Forbidden") {
                            ctx.block_channel(channel_id);
                            ctx.log(format!("[!] #{}: không có quyền truy cập (403), tự động bỏ qua kênh này.", channel_name));
                        } else if err.starts_with("AUTH:") {
                            ctx.fatal("Token bị Discord từ chối (401). Tài khoản này đã dừng, hãy kiểm tra lại token.");
                        } else if err.starts_with("RATE:") {
                            let secs = ctx.strike();
                            ctx.log(format!("[!] #{}: bị giới hạn tốc độ khi đọc tin nhắn, nghỉ {}s.", channel_name, secs));
                        } else {
                            ctx.log(format!("#{}: {}", channel_name, err));
                        }
                        break;
                    }
                };

                if page.is_empty() {
                    // Đã quét hết toàn bộ tin nhắn tới đầu kênh -> Lưu hoàn thành
                    ctx.config_mgr.update_channel_progress(&key, channel_name, |p| {
                        p.history_done = true;
                        p.history_cursor = None;
                        p.updated_at = Some(Local::now().format("%H:%M:%S").to_string());
                    });
                    break;
                }

                let page_len = page.len() as u32;
                total_messages_scanned += page_len;

                if newest_msg_id.is_none() {
                    newest_msg_id = page.first().map(|m| m.id.clone());
                }

                let oldest_id = page.last().and_then(|m| m.id.parse::<u64>().ok());

                let (reacted, _, had_error) =
                    Self::react_messages(ctx, guild_id, channel_id, channel_name, page, &settings, &config).await;

                total_channel_reacted += reacted;

                // Lưu tiến trình LẬP TỨC sau mỗi trang 100 tin đã duyệt
                let reached_start = page_len < fetch_limit;
                let oldest_str = oldest_id.map(|o| o.to_string());
                let newest_str = newest_msg_id.clone();
                ctx.config_mgr.update_channel_progress(&key, channel_name, |p| {
                    p.total_reacted += reacted;
                    if let Some(ref id) = newest_str {
                        p.last_reacted_id = Some(id.clone());
                    }
                    if reached_start {
                        p.history_done = true;
                        p.history_cursor = None;
                    } else if let Some(ref o) = oldest_str {
                        p.history_cursor = Some(o.clone());
                    }
                    p.updated_at = Some(Local::now().format("%H:%M:%S").to_string());
                });

                if had_error || !ctx.alive() || ctx.cooldown_left() > 0.0 {
                    break;
                }

                // Nếu trang trả về ít hơn số lượng yêu cầu -> Đã đến tin đầu tiên của kênh
                if reached_start {
                    break;
                }

                if let Some(oldest) = oldest_id {
                    before_cursor = Some(oldest);
                } else {
                    break;
                }
            }
        } else {
            // ── CHẾ ĐỘ QUÉT TIN MỚI (chỉ lấy tin nhắn sau last_reacted_id) ──
            let fetch_limit = max_msgs.unwrap_or(100).min(100);
            let page = match ctx.client.get_messages(channel_id, fetch_limit, last_id).await {
                Ok(msgs) => msgs,
                Err(err) => {
                    if err.contains("403") || err.contains("Forbidden") {
                        ctx.block_channel(channel_id);
                        ctx.log(format!("[!] #{}: không có quyền truy cập (403), tự động bỏ qua kênh này.", channel_name));
                    } else if err.starts_with("AUTH:") {
                        ctx.fatal("Token bị Discord từ chối (401). Tài khoản này đã dừng, hãy kiểm tra lại token.");
                    } else if err.starts_with("RATE:") {
                        let secs = ctx.strike();
                        ctx.log(format!("[!] #{}: bị giới hạn tốc độ khi đọc tin nhắn, nghỉ {}s.", channel_name, secs));
                    } else {
                        ctx.log(format!("#{}: {}", channel_name, err));
                    }
                    return 0;
                }
            };

            if !page.is_empty() {
                newest_msg_id = page.first().map(|m| m.id.clone());
                let (reacted, _, _) =
                    Self::react_messages(ctx, guild_id, channel_id, channel_name, page, &settings, &config).await;
                total_channel_reacted += reacted;

                let newest_str = newest_msg_id.clone();
                ctx.config_mgr.update_channel_progress(&key, channel_name, |p| {
                    p.total_reacted += reacted;
                    if let Some(ref id) = newest_str {
                        p.last_reacted_id = Some(id.clone());
                    }
                    p.updated_at = Some(Local::now().format("%H:%M:%S").to_string());
                });
            }
        }

        if total_channel_reacted > 0 || newest_msg_id.is_some() {
            let n = total_channel_reacted;
            let newest = newest_msg_id;
            ctx.config_mgr.update_channel_progress(&key, channel_name, |p| {
                p.total_reacted += n;
                if let Some(id) = newest {
                    p.last_reacted_id = Some(id);
                }
            });
        }

        if total_channel_reacted > 0 {
            ctx.config_mgr.save_stats();
        }

        total_channel_reacted
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
