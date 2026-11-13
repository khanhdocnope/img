# HƯỚNG DẪN DỰ ÁN DISCORD AUTO-REACTOR BẢN RUST (TAURI V2 MOBILE)

Phiên bản **Rust (Tauri v2 Mobile)** mang lại những ưu điểm vượt trội:
- **Tiêu thụ RAM siêu thấp**: Chỉ **15 – 35 MB** RAM (thay vì 150 – 250 MB của Python), giúp Android không bao giờ tắt ứng dụng khi chạy nền lâu.
- **Tiết kiệm Pin tối đa**: Dùng Tokio asynchronous I/O, không hao tổn chu kỳ CPU trong thời gian chờ delay.
- **Dung lượng APK nhẹ**: Chỉ khoảng **15 MB** (không cần đóng gói bộ thông dịch Python).
- **Giao diện hiện đại**: Sử dụng System WebView có sẵn trên Android kết hợp Dark Mode chuẩn Discord.

---

## 1. CẤU TRÚC MÃ NGUỒN

```
mmmmmmm/
├── src-tauri/
│   ├── Cargo.toml         # Cấu hình dependency Rust (tokio, reqwest, serde, tauri)
│   ├── tauri.conf.json    # Cấu hình ứng dụng Tauri Mobile
│   ├── capabilities/      # Quyền hạn API Tauri v2
│   └── src/
│       ├── main.rs        # Entry point Desktop/Android
│       ├── lib.rs         # Cầu nối Tauri Commands và Quản lý AppState
│       ├── models.rs      # Cấu trúc dữ liệu Discord Message, Channel, Category, Config
│       ├── config.rs      # Quản lý đọc/ghi JSON an toàn (đa tài khoản, tiến trình)
│       └── discord/
│           ├── client.rs  # Client REST API Discord tùy chỉnh cho User Token (Rate Limit 429 backoff)
│           └── engine.rs  # Vòng lặp quét kênh, kiểm tra emoji và thả reaction đa luồng
├── ui/                    # Giao diện Webview di động
│   ├── index.html         # Bố cục 5 Tabs (Bảng tin, Tài khoản, Cài đặt, Danh mục, Bộ lọc)
│   ├── style.css          # Giao diện Dark Mode cảm ứng mượt mà
│   └── app.js             # Kết nối trực tiếp với backend Rust
└── .github/
    └── workflows/
        └── build-rust-apk.yml  # Tự động biên dịch APK qua GitHub Actions
```

---

## 2. CHẠY THỬ NGHIỆM TRÊN MÁY TÍNH (WINDOWS / LINUX / MACOS)

Do máy của bạn đã cài sẵn Rust, bạn có thể chạy thử trực tiếp trên máy tính:

```bash
cargo run --manifest-path src-tauri/Cargo.toml
```

Giao diện sẽ hiển thị cửa sổ điện thoại mô phỏng với kích thước chuẩn, bạn có thể nhập Token, chỉnh sửa danh mục và test thả reaction trực tiếp!

---

## 3. TỰ ĐỘNG BIÊN DỊCH FILE APK QUA GITHUB ACTIONS

Workflow `.github/workflows/build-rust-apk.yml` đã được cấu hình sẵn. Khi bạn push code lên GitHub:
1. GitHub Actions sẽ tự động cài đặt Android SDK, NDK 26 và Rust target `aarch64-linux-android`.
2. Biên dịch bản Release APK với mức tối ưu hóa cao nhất.
3. Xuất file APK tại mục **Artifacts** của repository để bạn tải về điện thoại.
