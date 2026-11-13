use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotSettings {
    pub delay: f64,
    pub max_messages_per_channel: Option<u32>,
    pub skip_bot_messages: bool,
    pub full_scan: bool,
    pub concurrent_channels: usize,
}

impl Default for BotSettings {
    fn default() -> Self {
        Self {
            delay: 1.0,
            max_messages_per_channel: None,
            skip_bot_messages: true,
            full_scan: true,
            concurrent_channels: 4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryConfig {
    pub id: u64,
    pub name: String,
    pub excluded_channels: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub accounts: Vec<Account>,
    pub active_account_id: Option<String>,
    pub bot_settings: BotSettings,
    pub categories: Vec<CategoryConfig>,
    pub emoji_whitelist: Vec<String>,
    pub emoji_blacklist: Vec<String>,
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
    pub total_channels_scanned: usize,
    pub active_account_name: String,
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
pub struct DiscordMessage {
    pub id: String,
    pub channel_id: String,
    pub author: DiscordUser,
    #[serde(default)]
    pub reactions: Vec<DiscordReactionItem>,
}
