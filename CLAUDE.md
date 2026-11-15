# CLAUDE.md — Discord Reactor

Ứng dụng tự động thả reaction (và gửi emoji định kỳ) trên Discord bằng token người dùng.
Viết bằng **Tauri v2**: backend Rust, giao diện HTML/CSS/JS thuần (không framework, không npm).
Chạy trên **Android (APK)** và **Windows (EXE)**. Ngôn ngữ giao diện và log: tiếng Việt.

## Cấu trúc

```
src-tauri/            Backend Rust (crate discord-reactor, lib: discord_reactor_lib)
  src/lib.rs          Đăng ký Tauri commands, AppState, forward log ra UI
  src/models.rs       Config, Account, CategoryConfig, EmojiSenderSettings, EngineStatus, ...
  src/config.rs       ConfigManager: đọc/ghi config.json, progress.json, stats.json
  src/discord/client.rs   HTTP client tới Discord REST v10 (reqwest + rustls)
  src/discord/engine.rs   Vòng lặp bot: quét category -> react; luồng gửi emoji song song
ui/                   Frontend (index.html, style.css, app.js) — nạp qua frontendDist "../ui"
scripts/patch_android.py  Chạy sau `cargo tauri android init`: Foreground Service + AndroidBridge
.github/workflows/build.yml  CI: build APK + EXE rồi đăng lên GitHub Releases
```


Các thư mục/file `main.py`, `core/`, `data/`, `service.py`, `buildozer.spec`, `requirements.txt`
là **bản Python/Kivy cũ**, không còn dùng. Đừng sửa; chỉ tham khảo logic gốc khi cần.

## Dữ liệu lưu (app_data_dir của Tauri)

- `config.json`: tài khoản, category, bot_settings, bộ lọc emoji, emoji_sender
- `progress.json`: tiến trình quét từng kênh (`last_reacted_id`, `total_reacted`, trạng thái quét lịch sử). Khóa là ID kênh cho tài khoản đầu tiên (tương thích bản cũ) và `<id_tài_khoản>:<id_kênh>` cho tài khoản còn lại. Ghi qua `update_channel_progress` (nguyên tử, qua file tạm); nút "Đặt lại tiến trình" xóa file này
- `stats.json`: tổng reaction trọn đời (không bị xóa khi đặt lại tiến trình)

## Tính năng chính

- **React tự động**: quét các category đã cấu hình, react lại các emoji đã có trên tin nhắn. Whitelist/blacklist emoji (`emoji_whitelist`/`emoji_blacklist` trong `config.json`) vẫn được engine áp dụng nhưng đã bỏ tab giao diện; để trống = react tất cả. Bỏ qua tin của bot, quét toàn bộ hoặc chỉ tin mới, chạy song song nhiều kênh.
- **Chạy song song tối đa 2 tài khoản** (`Account.enabled`, `MAX_PARALLEL_ACCOUNTS` trong `config.rs`): mỗi tài khoản là một worker độc lập trong `engine.rs` với `WorkerCtx` riêng (client HTTP riêng, cờ chạy riêng, nhịp ngẫu nhiên riêng). Tài khoản thứ hai bắt đầu trễ 20–45s, độ trễ tối thiểu 0,8s và nửa số kênh song song. Chỉ tài khoản chính (`active_account_id`) gửi emoji định kỳ.
- **Bảo vệ rate limit / an toàn**: (1) 429 kéo dài -> worker nghỉ 60s -> 180s -> 600s; (2) `InvalidMeter` đếm chung phản hồi 401/403/429 trong 10 phút, đạt `INVALID_LIMIT` (120) thì mọi worker nghỉ 10 phút để tránh Cloudflare chặn IP; (3) 401 chỉ dừng tài khoản bị từ chối; (4) kênh 403 (không có quyền react) bị bỏ qua 30 phút. Tiền tố lỗi `AUTH:`/`RATE:`/`NET:` do `client.rs` sinh ra, engine dựa vào đó để phân loại.
- **Kiểm tra token**: lệnh Tauri `check_token` gọi `GET /users/@me`, trả `valid`/`invalid`/`unknown`. UI kiểm tra nền khi mở app và khi vào tab Tài khoản (cache 5 phút), kiểm tra trước khi lưu token mới (token hỏng bị chặn), tự lấy tên tài khoản Discord làm tên gợi nhớ, và chặn Bắt đầu nếu token đang dùng đã hỏng.
- **Quét lịch sử** (`scan_history`): mỗi lượt, mỗi kênh quét thêm một trang 100 tin cũ hơn (`before=`), lưu `history_cursor`/`history_done` trong `progress.json`. "Đặt lại tiến trình" sẽ quét lại lịch sử từ đầu.
- **Gửi emoji tự động**: gửi emoji tùy chỉnh của server vào 1 kênh cố định, mặc định mỗi 30s. Chọn ngẫu nhiên, không lặp trong 10 tin gần nhất của chính tài khoản. Mặc định bỏ emoji động (cần Nitro).
- **Chạy nền trên Android**: Foreground Service + wake lock + xin miễn tối ưu pin. UI gọi qua `window.AndroidBridge.startService()/stopService()`.
- **Thống kê**: số reaction phiên này, tổng trọn đời, số kênh đã quét.

## Quy ước và lưu ý quan trọng

- **ID Discord (snowflake) luôn là chuỗi ở phía JS.** Chúng lớn hơn 2^53 nên `parseInt`/Number làm tròn sai. Rust dùng `u64` nhưng serialize thành chuỗi (module `id_str` trong `models.rs`), vẫn đọc được config cũ dạng số. Không đưa `Number`/`parseInt` vào ID ở `app.js`.
- **Lỗi mạng** trả về `Err` bắt đầu bằng `"NET:"`. Engine dựa vào tiền tố này để dừng kênh, chờ rồi quét lại, thay vì bỏ sót tin nhắn. Giữ nguyên tiền tố khi thêm lệnh gọi HTTP mới.
- Khi một kênh gặp lỗi **tạm thời** (mất mạng `NET:` hoặc rate limit kéo dài), **không** cập nhật `last_reacted_id` để quét lại. Lỗi vĩnh viễn (400 đủ 20 emoji, emoji đã xóa) thì bỏ qua, nếu không một tin lỗi sẽ chặn cả kênh ở chế độ quét tin mới.
- Thêm trường mới vào `Config` phải có `#[serde(default)]`, nếu không file `config.json` cũ sẽ parse lỗi và bị ghi đè bằng mặc định (mất tài khoản).
- Tôn trọng rate limit: độ trễ giữa các reaction là ngẫu nhiên trong [`delay`, `delay_max`] (sàn 0,5s; mặc định 1,0–1,5s), nghỉ giữa các lượt quét ngẫu nhiên 5–10s; xử lý 429 bằng `retry_after`. Dùng `rand_between()` trong `engine.rs` cho mọi độ trễ mới.
- Mọi lệnh Tauri thêm mới phải được đăng ký trong `generate_handler!` ở `lib.rs`; tên tham số Rust (snake_case) khớp với khóa object phía JS khi `invoke` (Tauri tự đổi camelCase cho tham số rời, nhưng struct truyền nguyên khối thì giữ snake_case).
- UI: giữ nguyên các `id` phần tử mà `app.js` dùng; tab là `.screen`, chuyển tab bằng `.nav-item[data-tab]`. Màu nằm ở khối `:root` đầu `style.css`.

## Build

CI (khuyến nghị): push lên `main`/`master` hoặc chạy workflow `Build Windows EXE + Android APK`.
Kết quả ở trang Releases: `DiscordReactor.apk`, `DiscordReactor-setup.exe`, `DiscordReactor-portable.exe`.

Cục bộ (cần Rust và `cargo install tauri-cli --version "^2.0.0" --locked`):

```bash
cd src-tauri
cargo tauri dev                      # chạy thử trên máy tính
cargo tauri build --bundles nsis     # Windows
cargo tauri android init && cd .. && python3 scripts/patch_android.py && cd src-tauri
cargo tauri android build --apk --target aarch64   # Android (cần JDK 17, SDK 34, NDK 26.1.10909125)
```

`src-tauri/gen/android` được tạo lại mỗi lần `android init`, **đừng sửa trực tiếp**: mọi thay đổi Android đặt trong `scripts/patch_android.py`.

## Trạng thái kiểm thử

Mã Rust chưa từng được biên dịch trong môi trường phát triển hiện tại (không có cargo); lần build CI
đầu tiên là phép kiểm tra thật. Nếu CI lỗi biên dịch, sửa theo log rồi chạy lại.
Chỗ dễ lệch phiên bản Tauri nhất: `onWebViewCreate` trong `patch_android.py`.
