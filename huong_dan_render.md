# Hướng Dẫn Chạy Bot 24/7 Trên Render Free (Rust Cloud Web)

Ứng dụng Discord Auto-Reactor giờ đây hỗ trợ chạy như một **Web Service thuần Rust** trên nền tảng đám mây **Render Free**, hoạt động 24/7 mà không cần mở máy tính hay cắm sạc điện thoại.

---

## 🌟 Điểm nổi bật khi chạy trên Render Free

1. **Hiệu năng siêu nhẹ**: Được viết bằng Rust thuần, tiêu thụ chỉ **~15MB – 25MB RAM** (Render Free cho tối đa 512MB RAM, dư dả hơn 95%).
2. **Không tốn thời gian compile trên Render**: File thực thi Linux được **GitHub Actions build sẵn** với 4 vCPU và 16GB RAM. Khi Render deploy, Docker chỉ mất **10 giây** để tải binary và khởi chạy!
3. **Giao diện Web tương thích 100%**: Truy cập qua trình duyệt (`https://ten-ban.onrender.com`) với đầy đủ tính năng: thêm token, chọn kênh, xem reaction, stream log trực tiếp thời gian thực (SSE).
4. **Cơ chế chống ngủ đông (Keep-Alive)**: Tích hợp vòng lặp tự ping `/healthz` mỗi 10 phút để Render Free không bao giờ đưa app vào chế độ ngủ (spin down).

---

## 🚀 Cách 1: Triển khai 1-Click (Khuyên dùng - Render Blueprint)

1. Đăng ký/Đăng nhập tài khoản tại [render.com](https://render.com).
2. Ở trang chủ Dashboard, bấm nút **New +** (góc trên bên phải) ➔ Chọn **Blueprint**.
3. Chọn Repository GitHub của bạn (`khanhdocnope/img`).
4. Render sẽ tự động đọc file cấu hình `render.yaml` và `Dockerfile` có sẵn trong repo.
5. Nhấn **Apply**. Quá trình tải và khởi động chỉ mất khoảng **10 đến 30 giây**!

---

## 🛠️ Cách 2: Tạo Web Service thủ công

Nếu bạn muốn tự tạo Web Service:
1. Vào [dashboard.render.com](https://dashboard.render.com) ➔ Chọn **New +** ➔ **Web Service**.
2. Chọn kết nối với kho `khanhdocnope/img`.
3. Điền các thông số:
   - **Name**: `discord-reactor` (hoặc tên tuỳ thích).
   - **Region**: `Singapore` (độ trễ thấp về VN).
   - **Language / Runtime**: Chọn **Docker**.
   - **Instance Type**: Chọn **Free** ($0/tháng).
4. Bấm **Deploy Web Service**.

---

## 🔒 Lưu cấu hình vĩnh viễn (Tránh mất tài khoản khi Render khởi động lại)

Render Free sử dụng bộ nhớ tạm (ephemeral filesystem). Khi container khởi động lại hoặc deploy mới, dữ liệu ổ đĩa sẽ được làm mới.

Để lưu cấu hình vĩnh viễn:
1. Mở trang Web của Bot trên Render (ví dụ `https://discord-reactor.onrender.com`).
2. Nhập các tài khoản Discord và điều chỉnh cài đặt theo ý bạn.
3. Chuyển sang tab **Cài đặt** ➔ Nhấn nút **📋 Sao chép cấu hình (cho Render / Cloud)**.
4. Mở Render Dashboard của service ➔ Chuyển qua tab **Environment** ➔ Nhấn **Add Environment Variable**:
   - Tên biến: `BOT_CONFIG`
   - Giá trị: Dán nội dung JSON bạn vừa copy.
   - Thêm biến `AUTO_START`: giá trị `true` (nếu muốn bot tự động chạy ngay khi Render bật lên).
5. Nhấn **Save Changes**. Giờ đây mỗi lần khởi động, bot sẽ luôn nạp đúng tài khoản và tự động chạy!

---

## ⏰ Giữ Bot thức 24/7 (Phòng ngừa Render Free ngủ đông)

Render Free sẽ tắt service nếu không có ai truy cập sau 15 phút. Hệ thống đã có sẵn 2 tầng bảo vệ:

1. **Tự động (Tầng 1)**: Máy chủ Rust tự nhận diện biến `RENDER_EXTERNAL_URL` và tự gửi request ping `/healthz` mỗi 10 phút để luôn giữ máy chủ hoạt động.
2. **Dự phòng (Tầng 2 - Khuyên dùng thêm)**:
   - Đăng ký miễn phí tại [uptimerobot.com](https://uptimerobot.com).
   - Bấm **Add New Monitor**:
     - Monitor Type: `HTTP(s)`
     - Friendly Name: `Discord Reactor Render`
     - URL (or IP): `https://<ten-app-cua-ban>.onrender.com/healthz`
     - Monitoring Interval: `5 minutes`
   - Nhấn **Create Monitor**. UptimeRobot sẽ gửi tín hiệu mỗi 5 phút, đảm bảo bot chạy xuyên suốt 24/7/365 hoàn toàn miễn phí.
