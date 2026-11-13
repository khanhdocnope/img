"""
Android Foreground Service chạy ngầm cho Discord Auto-Reactor.
Được khởi chạy độc lập bởi Python-for-Android (p4a), giữ kết nối và CPU hoạt động 24/7.
"""

import sys
import os
import time
import asyncio
from pathlib import Path

# Thêm thư mục gốc vào PYTHONPATH
BASE_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(BASE_DIR))

from core.config import ConfigManager, DATA_DIR, LOG_FILE
from core.reactor import DiscordReactorEngine

STATUS_FILE = DATA_DIR / "service_status.json"
COMMAND_FILE = DATA_DIR / "service_cmd.json"

wake_lock = None


def setup_android_service():
    """Khởi tạo WakeLock và Foreground Notification trên Android."""
    global wake_lock
    try:
        from jnius import autoclass
        from android.runnable import run_on_ui_thread

        PythonService = autoclass("org.kivy.android.PythonService")
        service = PythonService.mService
        Context = autoclass("android.content.Context")
        PowerManager = autoclass("android.os.PowerManager")
        NotificationBuilder = autoclass("android.app.Notification$Builder")
        NotificationChannel = autoclass("android.app.NotificationChannel")
        NotificationManager = autoclass("android.app.NotificationManager")
        String = autoclass("java.lang.String")

        # 1. Acquire WakeLock (PARTIAL_WAKE_LOCK)
        pm = service.getSystemService(Context.POWER_SERVICE)
        wake_lock = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "DiscordReactor::WakeLock")
        wake_lock.acquire()
        print("[Service] WakeLock acquired thành công.")

        # 2. Tạo Notification Channel (Android 8.0+)
        channel_id = "discord_reactor_service_channel"
        channel_name = String("Discord Auto Reactor")
        importance = NotificationManager.IMPORTANCE_LOW
        channel = NotificationChannel(channel_id, channel_name, importance)
        channel.setDescription("Thông báo dịch vụ chạy ngầm Discord Auto-Reactor")

        nm = service.getSystemService(Context.NOTIFICATION_SERVICE)
        nm.createNotificationChannel(channel)

        # 3. Tạo Notification và chạy Foreground
        builder = NotificationBuilder(service, channel_id)
        builder.setContentTitle("Discord Auto-Reactor")
        builder.setContentText("Dịch vụ bot đang chạy ngầm...")
        builder.setSmallIcon(service.getApplicationInfo().icon)
        notification = builder.build()

        service.startForeground(1001, notification)
        print("[Service] startForeground kích hoạt thành công.")
    except Exception as e:
        print(f"[Service] Không chạy trên Android hoặc bỏ qua cấu hình Service: {e}")


def write_status(state: str, reacted_count: int, message: str = ""):
    try:
        import json
        data = {
            "state": state,
            "reacted_count": reacted_count,
            "message": message,
            "updated_at": time.time(),
        }
        with open(STATUS_FILE, "w", encoding="utf-8") as f:
            json.dump(data, f)
    except Exception:
        pass


def append_log(msg: str):
    try:
        with open(LOG_FILE, "a", encoding="utf-8") as f:
            f.write(f"{msg}\n")
    except Exception:
        pass


async def main():
    setup_android_service()
    config_mgr = ConfigManager()

    account = config_mgr.get_active_account()
    if not account or not account.get("token"):
        append_log("[LỖI Service] Không tìm thấy tài khoản kích hoạt hoặc Token trống!")
        write_status("stopped", 0, "Token trống")
        return

    progress = config_mgr.load_progress()

    def on_log(msg):
        print(msg)
        append_log(msg)

    def on_status(st, count):
        write_status(st, count)

    engine = DiscordReactorEngine(
        config=config_mgr.config,
        progress=progress,
        on_log=on_log,
        on_status=on_status,
        save_progress_fn=config_mgr.save_progress,
    )

    # Vòng lặp kiểm tra lệnh dừng từ giao diện UI
    async def command_watcher():
        while engine.running:
            if COMMAND_FILE.exists():
                try:
                    import json
                    with open(COMMAND_FILE, "r", encoding="utf-8") as f:
                        cmd = json.load(f)
                    if cmd.get("action") == "STOP":
                        on_log("[Service] Nhận lệnh DỪNG từ giao diện UI.")
                        engine.stop()
                        COMMAND_FILE.unlink(missing_ok=True)
                        break
                except Exception:
                    pass
            await asyncio.sleep(1.0)

    asyncio.create_task(command_watcher())
    await engine.start(account["token"])


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("[Service] Dừng bởi người dùng.")
    finally:
        write_status("stopped", 0, "Đã dừng")
        if wake_lock and wake_lock.isHeld():
            wake_lock.release()
