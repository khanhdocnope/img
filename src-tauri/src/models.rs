use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// ID Discord (snowflake) lớn hơn 2^53 nên JavaScript làm tròn sai khi là số.
/// Lưu/gửi dưới dạng chuỗi, nhưng vẫn đọc được file config cũ (dạng số).
mod id_str {
    use serde::{Deserialize, Deserializer, Serializer};

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumOrStr {
        Num(u64),
        Str(String),
    }

    impl NumOrStr {
        fn into_u64<E: serde::de::Error>(self) -> Result<u64, E> {
            match self {
                NumOrStr::Num(n) => Ok(n),
                NumOrStr::Str(s) => s.trim().parse::<u64>().map_err(E::custom),
            }
        }
    }

    pub fn serialize<S: Serializer>(v: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        NumOrStr::deserialize(d)?.into_u64()
    }

    pub fn ser_vec<S: Serializer>(v: &Vec<u64>, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(v.iter().map(|n| n.to_string()))
    }

    pub fn de_vec<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u64>, D::Error> {
        Vec::<NumOrStr>::deserialize(d)?
            .into_iter()
            .map(|x| x.into_u64())
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub token: String,
    /// Bật để tài khoản này chạy khi bấm Bắt đầu (tối đa 2 tài khoản song song)
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotSettings {
    /// Độ trễ tối thiểu giữa hai reaction (giây)
    pub delay: f64,
    /// Độ trễ tối đa (giây): mỗi reaction chờ một khoảng ngẫu nhiên trong [delay, delay_max]
    #[serde(default = "default_delay_max")]
    pub delay_max: f64,
    pub max_messages_per_channel: Option<u32>,
    pub skip_bot_messages: bool,
    pub full_scan: bool,
    pub concurrent_channels: usize,
    /// Quét ngược toàn bộ lịch sử kênh (từng trang 100 tin) để react mọi tin đã có reaction
    #[serde(default)]
    pub scan_history: bool,
    /// Tự động bấm nút (Auto Click Button / Đánh Boss ⚔️)
    #[serde(default = "default_true")]
    pub auto_button_enabled: bool,
    /// Từ khóa trong nhãn nút (mặc định: "Đánh Boss")
    #[serde(default = "default_button_label")]
    pub button_label_keyword: String,
    /// Từ khóa emoji của nút (mặc định: "⚔️")
    #[serde(default = "default_button_emoji")]
    pub button_emoji_keyword: String,
    /// Tiếp tục tiến trình quét cũ từ lần trước (Resume)
    #[serde(default = "default_true")]
    pub resume_progress: bool,
}

fn default_true() -> bool {
    true
}

fn default_button_label() -> String {
    "Đánh Boss".to_string()
}

fn default_button_emoji() -> String {
    "⚔️".to_string()
}

fn default_delay() -> f64 {
    0.3
}

fn default_delay_max() -> f64 {
    0.6
}

impl Default for BotSettings {
    fn default() -> Self {
        Self {
            delay: default_delay(),
            delay_max: default_delay_max(),
            max_messages_per_channel: None,
            skip_bot_messages: true,
            full_scan: true,
            concurrent_channels: 4,
            scan_history: false,
            auto_button_enabled: default_true(),
            button_label_keyword: default_button_label(),
            button_emoji_keyword: default_button_emoji(),
            resume_progress: default_true(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryConfig {
    #[serde(with = "id_str")]
    pub id: u64,
    pub name: String,
    #[serde(serialize_with = "id_str::ser_vec", deserialize_with = "id_str::de_vec")]
    pub excluded_channels: Vec<u64>,
}

/// Cấu hình tự động gửi emoji của server vào một kênh cố định.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmojiSenderSettings {
    #[serde(default)]
    pub enabled: bool,
    /// ID kênh (dạng chuỗi để không mất độ chính xác ở JS). Rỗng = chưa cấu hình.
    #[serde(default)]
    pub channel_id: String,
    /// Chu kỳ gửi (giây), khớp với slowmode của kênh.
    #[serde(default = "default_interval_secs")]
    pub interval_secs: u64,
    /// Cho phép gửi emoji động (GIF). Chỉ dùng được khi tài khoản có Nitro.
    #[serde(default)]
    pub allow_animated: bool,
}

fn default_interval_secs() -> u64 {
    30
}

impl Default for EmojiSenderSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            channel_id: String::new(),
            interval_secs: default_interval_secs(),
            allow_animated: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GuildEmoji {
    pub id: String,
    pub name: Option<String>,
    #[serde(default)]
    pub animated: bool,
    #[serde(default = "default_true")]
    pub available: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub accounts: Vec<Account>,
    pub active_account_id: Option<String>,
    pub bot_settings: BotSettings,
    pub categories: Vec<CategoryConfig>,
    pub emoji_whitelist: Vec<String>,
    pub emoji_blacklist: Vec<String>,
    #[serde(default)]
    pub emoji_sender: EmojiSenderSettings,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            accounts: Vec::new(),
            active_account_id: None,
            bot_settings: BotSettings::default(),
            categories: vec![CategoryConfig {
                id: 1281146625435369472,
                name: "Mặc định".to_string(),
                excluded_channels: vec![
                    1229710841927045222,
                    1246901718143205437,
                    1516859270140792842,
                    1516859246476787933,
                    1517099249257414777,
                    1196522451857842287,
                    1232731962834161775,
                    1437860347032895518,
                    1390315019006251008,
                    1281171567513960461,
                    1357284374684500159,
                ],
            }],
            emoji_whitelist: Vec::new(),
            emoji_blacklist: Vec::new(),
            emoji_sender: EmojiSenderSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChannelProgress {
    pub name: String,
    pub last_reacted_id: Option<String>,
    pub total_reacted: u64,
    /// ID tin cũ nhất đã quét trong lượt quét lịch sử (trang kế tiếp lấy các tin cũ hơn ID này)
    #[serde(default)]
    pub history_cursor: Option<String>,
    /// Đã quét tới tin đầu tiên của kênh
    #[serde(default)]
    pub history_done: bool,
    /// Thời gian cập nhật gần nhất
    #[serde(default)]
    pub updated_at: Option<String>,
}

pub type ProgressMap = HashMap<String, ChannelProgress>;

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Trạng thái của từng tài khoản đang chạy
pub struct WorkerStatus {
    pub account_id: String,
    pub name: String,
    /// starting | running | cooldown | error | stopped
    pub state: String,
    pub note: String,
    pub reactions: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatus {
    pub is_running: bool,
    pub session_reactions: u64,
    pub total_reactions: u64,
    pub total_channels_scanned: usize,
    pub active_account_name: String,
    pub workers: Vec<WorkerStatus>,
    #[serde(default)]
    pub progress_list: Vec<ChannelProgress>,
}

// --- Discord API Payloads ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
    pub discriminator: Option<String>,
    pub global_name: Option<String>,
    #[serde(default)]
    pub bot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordChannel {
    pub id: String,
    #[serde(rename = "type")]
    pub channel_type: u8,
    pub name: Option<String>,
    pub parent_id: Option<String>,
    pub guild_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordEmoji {
    pub id: Option<String>,
    pub name: Option<String>,
}

impl DiscordEmoji {
    pub fn to_reaction_param(&self) -> Option<String> {
        match (&self.id, &self.name) {
            (Some(id), Some(name)) => Some(format!("{}:{}", name, id)),
            (None, Some(name)) => Some(name.clone()),
            _ => None,
        }
    }

    pub fn to_display_string(&self) -> String {
        self.name.clone().unwrap_or_else(|| "emoji".to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordReactionItem {
    pub count: u32,
    pub me: bool,
    pub emoji: DiscordEmoji,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageComponent {
    #[serde(rename = "type")]
    pub component_type: u8,
    #[serde(default)]
    pub custom_id: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub style: Option<u8>,
    #[serde(default)]
    pub emoji: Option<DiscordEmoji>,
    #[serde(default)]
    pub disabled: Option<bool>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub components: Vec<MessageComponent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordMessage {
    pub id: String,
    pub channel_id: String,
    #[serde(default)]
    pub guild_id: Option<String>,
    pub author: DiscordUser,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub reactions: Vec<DiscordReactionItem>,
    #[serde(default)]
    pub components: Vec<MessageComponent>,
    #[serde(default)]
    pub application_id: Option<String>,
}

/// Kết quả kiểm tra một Discord token
#[derive(Debug, Clone, Serialize)]
pub struct TokenCheck {
    /// "valid" = còn dùng được, "invalid" = hết hạn/bị khóa, "unknown" = chưa xác định (mạng, rate limit...)
    pub status: String,
    pub username: Option<String>,
    pub message: String,
}
