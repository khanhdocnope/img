# HƯỚNG DẪN BUILD FILE APK CHO ĐIỆN THOẠI ANDROID

Có 2 phương thức cực kỳ nhanh và hoàn toàn **miễn phí** để build mã nguồn này thành file cài đặt `.apk`:

---

## CÁCH 1: DÙNG GOOGLE COLAB (CỰC NHANH - 5 ĐẾN 10 PHÚT LÀ CÓ FILE APK)

Bạn không cần cài bất kỳ phần mềm lập trình nào trên máy tính, mọi thao tác build diễn ra trên máy chủ đám mây của Google.

### Bước 1: Chuẩn bị file zip
Nén toàn bộ các file trong thư mục dự án này thành file `app.zip` (bao gồm: `main.py`, `service.py`, `buildozer.spec`, thư mục `core/`, `ui/`).

### Bước 2: Mở Google Colab
1. Truy cập vào: **https://colab.research.google.com/**
2. Bấm **New notebook** (Sổ tay mới).

### Bước 3: Dán đoạn lệnh (Prompt) sau vào ô Code và bấm Run:

```bash
# 1. Cài đặt các gói phụ thuộc hệ thống
!sudo apt update
!sudo apt install -y git zip unzip openjdk-17-jdk python3-pip autoconf libtool pkg-config zlib1g-dev libncurses5-dev libncursesw5-dev libtinfo5 cmake libffi-dev libssl-dev

# 2. Cài đặt Buildozer và Cython
!pip install --upgrade pip
!pip install cython==0.29.36
!pip install git+https://github.com/kivy/buildozer.git@master

# 3. Tải file mã nguồn lên (hoặc clone từ GitHub)
from google.colab import files
import zipfile
import os

print("Vui lòng chọn file app.zip vừa nén:")
uploaded = files.upload()

for filename in uploaded.keys():
    if filename.endswith(".zip"):
        with zipfile.ZipFile(filename, 'r') as zip_ref:
            zip_ref.extractall(".")
        print("Đã giải nén mã nguồn thành công!")

# 4. Tiến hành biên dịch APK với Buildozer
!buildozer -v android debug

# 5. Tải file APK trực tiếp về máy tính / điện thoại
import glob
apk_files = glob.glob("bin/*.apk")
if apk_files:
    print(f"Build thành công! Đang tải về: {apk_files[0]}")
    files.download(apk_files[0])
else:
    print("Có lỗi trong quá trình build, vui lòng kiểm tra log ở trên.")
```

---

## CÁCH 2: DÙNG GITHUB ACTIONS (TỰ ĐỘNG HÓA 100%)

Dự án đã được cấu hình sẵn file workflow tại `.github/workflows/build-apk.yml`.

### Các bước thực hiện:
1. Tạo một repository mới trên **GitHub** (có thể chọn chế độ **Private** để bảo mật).
2. Đẩy (Push) toàn bộ thư mục này lên repository vừa tạo:
   ```bash
   git init
   git add .
   git commit -m "Khoi tao Discord Reactor APK"
   git branch -M main
   git remote add origin https://github.com/<tai-khoan-cua-ban>/<ten-repo>.git
   git push -u origin main
   ```
3. Truy cập vào mục **Actions** trên thanh menu của GitHub repository.
4. Bạn sẽ thấy luồng **Build Android APK** đang tự động chạy.
5. Sau khoảng 8-12 phút, workflow hoàn thành, bạn chỉ cần bấm vào mục **Artifacts** để tải file `DiscordReactor-APK` về máy và cài đặt.

---

## CÁC LƯU Ý KHI CÀI ĐẶT TRÊN ĐIỆN THOẠI

1. **Cài đặt file APK**:
   - Khi mở file `.apk`, điện thoại sẽ hỏi "Cho phép cài đặt từ nguồn không xác định" -> Bật **Cho phép**.
2. **Cấp quyền thông báo**:
   - Khi mở app lần đầu, cho phép ứng dụng gửi **Thông báo** (Notification) để app có thể hiển thị thanh trạng thái Foreground Service.
3. **Tắt tối ưu hóa pin (RẤT QUAN TRỌNG để bot không bị tắt khi khóa màn hình)**:
   - Vào **Cài đặt điện thoại** -> **Ứng dụng** -> Chọn **Discord Reactor**.
   - Vào mục **Pin / Tiết kiệm pin** -> Chọn **Không giới hạn** (No restrictions) hoặc **Không tối ưu hóa pin**.
   - Bật **Tự khởi chạy** (Autostart) nếu máy là Xiaomi (MIUI/HyperOS), Oppo, Vivo hoặc Huawei.
