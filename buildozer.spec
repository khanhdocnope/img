[app]

# Tên ứng dụng hiển thị trên màn hình điện thoại
title = Discord Reactor

# Tên gói (viết thường, không dấu cách)
package.name = discordreactor

# Tên định danh tổ chức
package.domain = org.reactor

# Thư mục chứa mã nguồn
source.dir = .

# Các định dạng file được đóng gói vào APK
source.include_exts = py,png,jpg,kv,atlas,json,txt

# Phiên bản ứng dụng
version = 1.0.0

# Các thư viện phụ thuộc của Python
requirements = python3,kivy,discord.py-self,aiohttp,certifi,chardet,idna,multidict,yarl,pyjnius

# Hướng màn hình (portrait: màn hình dọc)
orientation = portrait

# Đăng ký dịch vụ chạy ngầm Android Foreground Service
services = DiscordReactor:service.py

# Quyền truy cập Android bắt buộc
android.permissions = INTERNET,ACCESS_NETWORK_STATE,WAKE_LOCK,FOREGROUND_SERVICE,FOREGROUND_SERVICE_SPECIAL_USE,POST_NOTIFICATIONS

# Target Android API
android.api = 34
android.minapi = 26
android.ndk = 25b
android.accept_sdk_license = True

# Kiến trúc CPU hỗ trợ cho Android (arm64-v8a phổ biến nhất trên 99% smartphone)
android.archs = arm64-v8a, armeabi-v7a

# Cho phép ứng dụng nhận thông báo
android.logcat_filters = *:S python:D

# Bật hỗ trợ service foreground
android.entrypoint = org.kivy.android.PythonActivity

[buildozer]

# Mức độ log khi build (2 = chi tiết nhất)
log_level = 2

# Cảnh báo khi chạy quyền root
warn_on_root = 1
