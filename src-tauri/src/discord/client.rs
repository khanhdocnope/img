use crate::models::{DiscordChannel, DiscordGuild, DiscordMessage, DiscordUser, GuildEmoji};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use serde::Deserialize;
use std::time::Duration;
use tokio::time::sleep;

const BASE_URL: &str = "https://discord.com/api/v10";
const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36";

#[derive(Debug, Deserialize)]
struct RateLimitResponse {
    #[allow(dead_code)]
    pub message: Option<String>,
    pub retry_after: Option<f64>,
}

/// Kết quả gửi tin nhắn
#[derive(Debug)]
pub enum SendOutcome {
    Sent,
    /// Bị giới hạn tốc độ / slowmode: số giây cần chờ
    RateLimited(f64),
    /// Không có quyền nhắn trong kênh
    Forbidden,
}

#[derive(Clone)]
pub struct DiscordClient {
    client: reqwest::Client,
    token: String,
}

impl DiscordClient {
    pub fn new(token: String) -> Result<Self, String> {
        let mut headers = HeaderMap::new();
        // User tokens không có prefix 'Bot '
        let mut auth_val = HeaderValue::from_str(&token)
            .map_err(|e| format!("Token chứa ký tự không hợp lệ: {}", e))?;
        auth_val.set_sensitive(true);
        headers.insert(AUTHORIZATION, auth_val);
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(DEFAULT_USER_AGENT),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(10))
            // Không tái dùng kết nối cũ: sau khi Android đóng băng app, kết nối trong pool đã chết
            .pool_max_idle_per_host(0)
            .tcp_keepalive(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Không thể khởi tạo HTTP Client: {}", e))?;

        Ok(Self { client, token })
    }

    pub fn get_token(&self) -> &str {
        &self.token
    }

    /// Kiểm tra token và lấy thông tin tài khoản hiện tại
    pub async fn get_me(&self) -> Result<DiscordUser, String> {
        let url = format!("{}/users/@me", BASE_URL);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Lỗi kết nối tới Discord API: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Lỗi xác thực Token ({}) : {}", status, body));
        }

        resp.json::<DiscordUser>()
            .await
            .map_err(|e| format!("Không thể parse thông tin tài khoản: {}", e))
    }

    /// Lấy danh sách server mà tài khoản đang tham gia
    pub async fn get_user_guilds(&self) -> Result<Vec<DiscordGuild>, String> {
        let url = format!("{}/users/@me/guilds", BASE_URL);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("NET: Lỗi kết nối get_user_guilds: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Lỗi HTTP {} khi lấy danh sách server", resp.status()));
        }

        resp.json::<Vec<DiscordGuild>>()
            .await
            .map_err(|e| format!("Không thể parse danh sách server: {}", e))
    }

    /// Lấy thông tin kênh (để tìm guild_id của Category)
    pub async fn get_channel(&self, channel_id: u64) -> Result<DiscordChannel, String> {
        let url = format!("{}/channels/{}", BASE_URL, channel_id);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Lỗi kết nối get_channel: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Không tìm thấy kênh/category ID: {}", channel_id));
        }

        resp.json::<DiscordChannel>()
            .await
            .map_err(|e| format!("Không thể parse thông tin kênh {}: {}", channel_id, e))
    }

    /// Lấy danh sách tất cả các kênh trong Server (Guild)
    pub async fn get_guild_channels(&self, guild_id: &str) -> Result<Vec<DiscordChannel>, String> {
        let url = format!("{}/guilds/{}/channels", BASE_URL, guild_id);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Lỗi kết nối get_guild_channels: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Không thể lấy danh sách kênh của server {}: HTTP {}", guild_id, resp.status()));
        }

        resp.json::<Vec<DiscordChannel>>()
            .await
            .map_err(|e| format!("Không thể parse danh sách kênh server {}: {}", guild_id, e))
    }

    /// Lấy danh sách tin nhắn của một kênh
    pub async fn get_messages(
        &self,
        channel_id: u64,
        limit: u32,
        after: Option<u64>,
    ) -> Result<Vec<DiscordMessage>, String> {
        let mut url = format!("{}/channels/{}/messages?limit={}", BASE_URL, channel_id, limit.min(100));
        if let Some(after_id) = after {
            url.push_str(&format!("&after={}", after_id));
        }

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Lỗi đọc tin nhắn kênh {}: {}", channel_id, e))?;

        if !resp.status().is_success() {
            return Err(format!("Lỗi HTTP {} khi lấy tin nhắn kênh {}", resp.status(), channel_id));
        }

        resp.json::<Vec<DiscordMessage>>()
            .await
            .map_err(|e| format!("Không thể parse tin nhắn kênh {}: {}", channel_id, e))
    }

    /// Lấy danh sách emoji tùy chỉnh của server
    pub async fn get_guild_emojis(&self, guild_id: &str) -> Result<Vec<GuildEmoji>, String> {
        let url = format!("{}/guilds/{}/emojis", BASE_URL, guild_id);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("NET: Lỗi kết nối get_guild_emojis: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Không thể lấy emoji của server {}: HTTP {}", guild_id, resp.status()));
        }

        resp.json::<Vec<GuildEmoji>>()
            .await
            .map_err(|e| format!("Không thể parse danh sách emoji server {}: {}", guild_id, e))
    }

    /// Gửi một tin nhắn văn bản vào kênh
    pub async fn send_message(&self, channel_id: u64, content: &str) -> Result<SendOutcome, String> {
        let url = format!("{}/channels/{}/messages", BASE_URL, channel_id);

        // nonce dạng snowflake giống client Discord thật
        const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(DISCORD_EPOCH_MS);
        let nonce = (now_ms.saturating_sub(DISCORD_EPOCH_MS) << 22).to_string();

        let body = serde_json::json!({
            "content": content,
            "nonce": nonce,
            "tts": false,
            "flags": 0
        });

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("NET: Lỗi gửi tin nhắn: {}", e))?;

        let status = resp.status();
        if status.is_success() {
            return Ok(SendOutcome::Sent);
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = resp
                .json::<RateLimitResponse>()
                .await
                .ok()
                .and_then(|r| r.retry_after)
                .unwrap_or(5.0);
            return Ok(SendOutcome::RateLimited(retry_after));
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Ok(SendOutcome::Forbidden);
        }

        let err_body = resp.text().await.unwrap_or_default();
        let snippet: String = err_body.chars().take(120).collect();
        Err(format!("Lỗi HTTP {} khi gửi tin nhắn: {}", status, snippet))
    }

    /// Thêm reaction vào tin nhắn với cơ chế xử lý Rate Limit (429) lũy tiến tự động
    pub async fn add_reaction(
        &self,
        channel_id: u64,
        message_id: &str,
        emoji_param: &str,
    ) -> Result<bool, String> {
        let encoded_emoji = urlencoding::encode(emoji_param);
        let url = format!(
            "{}/channels/{}/messages/{}/reactions/{}/@me",
            BASE_URL, channel_id, message_id, encoded_emoji
        );

        let mut attempts = 0;
        loop {
            attempts += 1;
            if attempts > 6 {
                return Err("Đã vượt quá số lần thử lại tối đa do Rate Limit kéo dài.".to_string());
            }

            let resp = match self.client.put(&url).send().await {
                Ok(r) => r,
                Err(e) => {
                    // Lỗi mạng (app vừa bị treo / mất mạng tạm thời): chờ rồi thử lại
                    if attempts >= 6 {
                        // Tiền tố NET: để engine biết đây là lỗi mạng, không bỏ qua tin nhắn
                        return Err(format!("NET: Lỗi gửi reaction: {}", e));
                    }
                    sleep(Duration::from_secs((attempts as u64 * 3).min(15))).await;
                    continue;
                }
            };

            let status = resp.status();
            if status == reqwest::StatusCode::NO_CONTENT || status == reqwest::StatusCode::OK {
                return Ok(true);
            }

            if status == reqwest::StatusCode::FORBIDDEN {
                // Không có quyền react trong kênh này
                return Ok(false);
            }

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let mut retry_after = 2.0;
                if let Ok(rate_limit) = resp.json::<RateLimitResponse>().await {
                    if let Some(secs) = rate_limit.retry_after {
                        retry_after = secs;
                    }
                }
                let sleep_duration = Duration::from_secs_f64(retry_after + 0.5);
                sleep(sleep_duration).await;
                continue;
            }

            let err_body = resp.text().await.unwrap_or_default();
            return Err(format!("Lỗi HTTP {}: {}", status, &err_body[..err_body.len().min(80)]));
        }
    }
}
