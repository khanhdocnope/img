use crate::models::{DiscordChannel, DiscordMessage, DiscordUser, GuildEmoji, TokenCheck};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use parking_lot::Mutex;
use serde::Deserialize;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
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

/// Đếm các phản hồi lỗi 401/403/429 trong 10 phút gần nhất (chung cho mọi tài khoản).
/// Discord/Cloudflare chặn IP tạm thời khi có quá nhiều phản hồi loại này, nên engine
/// dùng bộ đếm này để chủ động tạm nghỉ trước khi bị chặn.
#[derive(Default)]
pub struct InvalidMeter {
    hits: Mutex<VecDeque<Instant>>,
}

impl InvalidMeter {
    const WINDOW: Duration = Duration::from_secs(600);

    fn prune(hits: &mut VecDeque<Instant>, now: Instant) {
        while let Some(first) = hits.front() {
            if now.saturating_duration_since(*first) > Self::WINDOW {
                hits.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn record(&self) {
        let now = Instant::now();
        let mut hits = self.hits.lock();
        hits.push_back(now);
        Self::prune(&mut hits, now);
    }

    pub fn recent(&self) -> usize {
        let mut hits = self.hits.lock();
        Self::prune(&mut hits, Instant::now());
        hits.len()
    }
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
    meter: Option<Arc<InvalidMeter>>,
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

        Ok(Self { client, token, meter: None })
    }

    /// Gắn bộ đếm phản hồi lỗi dùng chung giữa các tài khoản
    pub fn with_meter(mut self, meter: Arc<InvalidMeter>) -> Self {
        self.meter = Some(meter);
        self
    }

    /// Ghi nhận phản hồi 401/403/429 vào bộ đếm
    fn note(&self, status: reqwest::StatusCode) {
        if let Some(m) = &self.meter {
            if matches!(status.as_u16(), 401 | 403 | 429) {
                m.record();
            }
        }
    }

    /// Chuỗi lỗi có tiền tố để engine phân loại: AUTH (token bị từ chối), RATE (bị giới hạn tốc độ)
    fn http_err(status: reqwest::StatusCode, what: &str) -> String {
        match status.as_u16() {
            401 => format!("AUTH: Token bị Discord từ chối (401) {}", what),
            429 => format!("RATE: Bị giới hạn tốc độ (429) {}", what),
            _ => format!("Lỗi HTTP {} {}", status, what),
        }
    }

    pub fn get_token(&self) -> &str {
        &self.token
    }

    /// Kiểm tra token còn dùng được không (gọi GET /users/@me), không bao giờ trả Err
    pub async fn check_token(token: &str) -> TokenCheck {
        let make = |status: &str, username: Option<String>, message: &str| TokenCheck {
            status: status.to_string(),
            username,
            message: message.to_string(),
        };

        let client = match DiscordClient::new(token.trim().to_string()) {
            Ok(c) => c,
            Err(_) => return make("invalid", None, "Token chứa ký tự không hợp lệ."),
        };

        let url = format!("{}/users/@me", BASE_URL);
        let resp = match client.client.get(&url).send().await {
            Ok(r) => r,
            Err(_) => return make("unknown", None, "Không kết nối được tới Discord. Kiểm tra mạng."),
        };

        let status = resp.status();
        if status.is_success() {
            return match resp.json::<DiscordUser>().await {
                Ok(u) => {
                    let name = u.global_name.filter(|n| !n.is_empty()).unwrap_or(u.username);
                    make("valid", Some(name), "Token còn hoạt động.")
                }
                Err(_) => make("unknown", None, "Discord trả về dữ liệu không đọc được."),
            };
        }

        match status.as_u16() {
            401 => make("invalid", None, "Token đã hết hạn hoặc tài khoản đã đăng xuất."),
            403 => make("invalid", None, "Tài khoản bị khóa hoặc cần xác minh."),
            429 => make("unknown", None, "Đang bị giới hạn tốc độ, hãy thử lại sau ít phút."),
            code => make("unknown", None, &format!("Discord trả về lỗi HTTP {}.", code)),
        }
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
            self.note(resp.status());
            if matches!(resp.status().as_u16(), 401 | 429) {
                return Err(Self::http_err(resp.status(), &format!("khi lấy kênh {}", channel_id)));
            }
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
            self.note(resp.status());
            return Err(Self::http_err(resp.status(), &format!("khi lấy danh sách kênh server {}", guild_id)));
        }

        resp.json::<Vec<DiscordChannel>>()
            .await
            .map_err(|e| format!("Không thể parse danh sách kênh server {}: {}", guild_id, e))
    }

    /// Lấy một trang tin nhắn cũ hơn `before` (None = trang mới nhất), mới nhất trước
    pub async fn get_messages_before(
        &self,
        channel_id: u64,
        limit: u32,
        before: Option<u64>,
    ) -> Result<Vec<DiscordMessage>, String> {
        let mut url = format!("{}/channels/{}/messages?limit={}", BASE_URL, channel_id, limit.min(100));
        if let Some(before_id) = before {
            url.push_str(&format!("&before={}", before_id));
        }

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("NET: Lỗi đọc tin nhắn kênh {}: {}", channel_id, e))?;

        if !resp.status().is_success() {
            self.note(resp.status());
            return Err(Self::http_err(resp.status(), &format!("khi lấy tin nhắn kênh {}", channel_id)));
        }

        resp.json::<Vec<DiscordMessage>>()
            .await
            .map_err(|e| format!("Không thể parse tin nhắn kênh {}: {}", channel_id, e))
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
            self.note(resp.status());
            return Err(Self::http_err(resp.status(), &format!("khi lấy tin nhắn kênh {}", channel_id)));
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
        self.note(status);
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(Self::http_err(status, "khi gửi tin nhắn"));
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
            self.note(status);

            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(Self::http_err(status, "khi thả reaction"));
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
            let snippet: String = err_body.chars().take(80).collect();
            return Err(format!("Lỗi HTTP {}: {}", status, snippet));
        }
    }
}
