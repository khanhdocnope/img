import os
import sys
import json
import time
import threading
import asyncio
from pathlib import Path

# Thêm đường dẫn gốc vào sys.path
BASE_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(BASE_DIR))

from kivy.app import App
from kivy.core.window import Window
from kivy.uix.boxlayout import BoxLayout
from kivy.uix.screenmanager import ScreenManager, Screen, SlideTransition
from kivy.uix.button import Button
from kivy.utils import get_color_from_hex
from kivy.graphics import Color, Rectangle

from core.config import ConfigManager, DATA_DIR, LOG_FILE
from core.reactor import DiscordReactorEngine
from ui.components import (
    StyledButton,
    COLOR_BG_DARK,
    COLOR_CARD,
    COLOR_PRIMARY,
    COLOR_TEXT_PRIMARY,
    COLOR_TEXT_MUTED,
)
from ui.views.dashboard import DashboardView
from ui.views.accounts import AccountsView
from ui.views.settings import SettingsView
from ui.views.categories import CategoriesView
from ui.views.filters import FiltersView

# Đặt kích thước cửa sổ giả lập tỉ lệ màn hình điện thoại khi chạy trên máy tính
if sys.platform in ["win32", "darwin", "linux"]:
    Window.size = (380, 680)

STATUS_FILE = DATA_DIR / "service_status.json"
COMMAND_FILE = DATA_DIR / "service_cmd.json"


class DiscordReactorApp(App):
    """Ứng dụng Kivy Discord Auto-Reactor cho Android & Desktop."""

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self.config_mgr = ConfigManager()
        self.is_android = self.check_is_android()

        # Biến điều khiển trên môi trường Desktop
        self.desktop_engine = None
        self.desktop_thread = None
        self.desktop_running = False
        self.desktop_stats = {"session_reacted": 0, "total_channels": 0}

    def check_is_android(self) -> bool:
        try:
            import jnius
            return True
        except ImportError:
            return False

    def build(self):
        self.title = "Discord Auto-Reactor"

        root = BoxLayout(orientation="vertical")
        with root.canvas.before:
            Color(*COLOR_BG_DARK)
            self._bg_rect = Rectangle(pos=root.pos, size=root.size)
        root.bind(pos=self._update_bg, size=self._update_bg)

        # 1. Screen Manager chứa các màn hình giao diện
        self.sm = ScreenManager(transition=SlideTransition(duration=0.15))

        # Khởi tạo các Views
        self.dashboard_view = DashboardView(app_controller=self)
        self.accounts_view = AccountsView(app_controller=self)
        self.settings_view = SettingsView(app_controller=self)
        self.categories_view = CategoriesView(app_controller=self)
        self.filters_view = FiltersView(app_controller=self)

        screen_dash = Screen(name="dashboard")
        screen_dash.add_widget(self.dashboard_view)
        self.sm.add_widget(screen_dash)

        screen_acc = Screen(name="accounts")
        screen_acc.add_widget(self.accounts_view)
        self.sm.add_widget(screen_acc)

        screen_set = Screen(name="settings")
        screen_set.add_widget(self.settings_view)
        self.sm.add_widget(screen_set)

        screen_cat = Screen(name="categories")
        screen_cat.add_widget(self.categories_view)
        self.sm.add_widget(screen_cat)

        screen_fil = Screen(name="filters")
        screen_fil.add_widget(self.filters_view)
        self.sm.add_widget(screen_fil)

        root.add_widget(self.sm)

        # 2. Bottom Navigation Bar điều hướng chuẩn di động
        nav_bar = self._create_bottom_nav()
        root.add_widget(nav_bar)

        return root

    def _update_bg(self, instance, value):
        self._bg_rect.pos = instance.pos
        self._bg_rect.size = instance.size

    def _create_bottom_nav(self):
        nav_box = BoxLayout(
            orientation="horizontal",
            size_hint_y=None,
            height="56dp",
            spacing="2dp",
            padding=["4dp", "4dp", "4dp", "4dp"],
        )
        with nav_box.canvas.before:
            Color(*COLOR_CARD)
            self._nav_rect = Rectangle(pos=nav_box.pos, size=nav_box.size)
        nav_box.bind(
            pos=lambda inst, v: setattr(self._nav_rect, "pos", inst.pos),
            size=lambda inst, v: setattr(self._nav_rect, "size", inst.size),
        )

        tabs = [
            ("dashboard", "Bảng tin\n(Dash)"),
            ("accounts", "Tài khoản\n(Accs)"),
            ("settings", "Cài đặt\n(Config)"),
            ("categories", "Danh mục\n(Cats)"),
            ("filters", "Emoji\n(Lọc)"),
        ]

        self.nav_buttons = {}
        for screen_name, label_text in tabs:
            btn = Button(
                text=label_text,
                font_size="11sp",
                bold=True,
                background_color=(0, 0, 0, 0),
                color=COLOR_PRIMARY if screen_name == "dashboard" else COLOR_TEXT_MUTED,
            )
            btn.bind(on_press=lambda inst, name=screen_name: self.switch_tab(name))
            self.nav_buttons[screen_name] = btn
            nav_box.add_widget(btn)

        return nav_box

    def switch_tab(self, screen_name):
        self.sm.current = screen_name
        for name, btn in self.nav_buttons.items():
            btn.color = COLOR_PRIMARY if name == screen_name else COLOR_TEXT_MUTED

    # --- Điều khiển Bot & Service Lifecycle ---
    def is_bot_running(self) -> bool:
        if self.is_android:
            # Đọc trạng thái từ file status do Android Foreground Service cập nhật
            if STATUS_FILE.exists():
                try:
                    with open(STATUS_FILE, "r", encoding="utf-8") as f:
                        data = json.load(f)
                    # Nếu file được cập nhật trong vòng 5 giây gần nhất và trạng thái là 'running'
                    if data.get("state") in ["running", "starting"]:
                        if time.time() - data.get("updated_at", 0) < 15:
                            return True
                except Exception:
                    pass
            return False
        else:
            return self.desktop_running

    def get_current_stats(self) -> dict:
        if self.is_android:
            if STATUS_FILE.exists():
                try:
                    with open(STATUS_FILE, "r", encoding="utf-8") as f:
                        data = json.load(f)
                    return {
                        "session_reacted": data.get("reacted_count", 0),
                        "total_channels": len(self.config_mgr.load_progress()),
                    }
                except Exception:
                    pass
            return {"session_reacted": 0, "total_channels": 0}
        else:
            self.desktop_stats["total_channels"] = len(self.config_mgr.load_progress())
            return self.desktop_stats

    def start_bot(self) -> bool:
        account = self.config_mgr.get_active_account()
        if not account or not account.get("token"):
            self._append_local_log("[LỖI] Bạn chưa chọn tài khoản hoặc chưa nhập Token!")
            return False

        if self.is_android:
            # Khởi chạy Android Service
            try:
                from jnius import autoclass
                mActivity = autoclass("org.kivy.android.PythonActivity").mActivity
                service_class = autoclass("org.kivy.android.PythonService")
                # Xóa lệnh dừng cũ nếu có
                COMMAND_FILE.unlink(missing_ok=True)
                service_class.start(mActivity, "")
                self._append_local_log("[Android] Đã kích hoạt Foreground Service.")
                return True
            except Exception as e:
                self._append_local_log(f"[LỖI Khởi động Service Android] {e}")
                return False
        else:
            # Chạy nền qua asyncio thread trên Desktop
            self.desktop_running = True
            token = account["token"]

            def run_in_thread():
                loop = asyncio.new_event_loop()
                asyncio.set_event_loop(loop)

                def on_log(msg):
                    print(msg)
                    self._append_local_log(msg)

                def on_status(st, count):
                    self.desktop_stats["session_reacted"] = count
                    if st == "stopped":
                        self.desktop_running = False

                self.desktop_engine = DiscordReactorEngine(
                    config=self.config_mgr.config,
                    progress=self.config_mgr.load_progress(),
                    on_log=on_log,
                    on_status=on_status,
                    save_progress_fn=self.config_mgr.save_progress,
                )

                try:
                    loop.run_until_complete(self.desktop_engine.start(token))
                except Exception as e:
                    self._append_local_log(f"[LỖI Thread Bot] {e}")
                finally:
                    self.desktop_running = False

            self.desktop_thread = threading.Thread(target=run_in_thread, daemon=True)
            self.desktop_thread.start()
            self._append_local_log("[Desktop] Đang kết nối bot...")
            return True

    def stop_bot(self):
        if self.is_android:
            # Gửi lệnh dừng tới service
            try:
                with open(COMMAND_FILE, "w", encoding="utf-8") as f:
                    json.dump({"action": "STOP", "time": time.time()}, f)
                self._append_local_log("[Android] Đã gửi tín hiệu dừng tới Service.")
            except Exception as e:
                self._append_local_log(f"[LỖI Gửi lệnh dừng] {e}")
        else:
            if self.desktop_engine:
                self.desktop_engine.stop()
            self.desktop_running = False
            self._append_local_log("[Desktop] Đã yêu cầu dừng bot.")

    def _append_local_log(self, msg: str):
        try:
            timestamp = time.strftime("%H:%M:%S")
            with open(LOG_FILE, "a", encoding="utf-8") as f:
                f.write(f"[{timestamp}] {msg}\n")
        except Exception:
            pass


if __name__ == "__main__":
    DiscordReactorApp().run()
