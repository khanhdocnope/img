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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotSettings {
    pub delay: f64,
    #[serde(default)]
    pub delay_max: Option<f64>,
    pub max_messages_per_channel: Option<u32>,
    pub skip_bot_messages: bool,
    pub full_scan: bool,
    pub concurrent_channels: usize,
}

impl Default for BotSettings {
    fn default() -> Self {
        Self {
            delay: 1.0,
            delay_max: Some(2.5),
            max_messages_per_channel: None,
            skip_bot_messages: true,
            full_scan: true,
            concurrent_channels: 4,
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
}

pub type ProgressMap = HashMap<String, ChannelProgress>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatus {
    pub is_running: bool,
    pub session_reactions: u64,
    pub total_reactions: u64,
    pub total_channels_scanned: usize,
    pub active_account_name: String,
}

// --- Discord API Payloads ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordGuild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
}

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
pub struct DiscordMessage {
    pub id: String,
    pub channel_id: String,
    pub author: DiscordUser,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub reactions: Vec<DiscordReactionItem>,
}
