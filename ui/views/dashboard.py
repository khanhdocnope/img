import os
import json
import time
from pathlib import Path
from kivy.uix.boxlayout import BoxLayout
from kivy.uix.scrollview import ScrollView
from kivy.uix.label import Label
from kivy.uix.textinput import TextInput
from kivy.clock import Clock
from kivy.utils import get_color_from_hex

from ui.components import (
    Card,
    StyledButton,
    SectionHeader,
    COLOR_BG_DARK,
    COLOR_SUCCESS,
    COLOR_DANGER,
    COLOR_TEXT_PRIMARY,
    COLOR_TEXT_MUTED,
    COLOR_INPUT_BG,
)
from core.config import LOG_FILE, DATA_DIR

STATUS_FILE = DATA_DIR / "service_status.json"
COMMAND_FILE = DATA_DIR / "service_cmd.json"


class DashboardView(BoxLayout):
    """Màn hình chính Dashboard: Trạng thái, Điều khiển Start/Stop, Log thời gian thực."""

    def __init__(self, app_controller, **kwargs):
        super().__init__(**kwargs)
        self.app_controller = app_controller
        self.orientation = "vertical"
        self.spacing = "12dp"
        self.padding = ["12dp", "12dp", "12dp", "12dp"]

        # --- 1. Header Card: Trạng thái và Tài khoản hiện tại ---
        self.status_card = Card(orientation="vertical", size_hint_y=None, height="90dp", padding="12dp", spacing="6dp")
        
        status_row = BoxLayout(orientation="horizontal", size_hint_y=None, height="30dp")
        self.lbl_account = Label(
            text="Tài khoản: Chưa chọn",
            font_size="15sp",
            bold=True,
            color=COLOR_TEXT_PRIMARY,
            halign="left",
            valign="middle",
        )
        self.lbl_account.bind(size=self.lbl_account.setter("text_size"))
        
        self.lbl_status = Label(
            text="● ĐÃ DỪNG",
            font_size="14sp",
            bold=True,
            color=COLOR_DANGER,
            size_hint_x=None,
            width="120dp",
            halign="right",
            valign="middle",
        )
        self.lbl_status.bind(size=self.lbl_status.setter("text_size"))
        
        status_row.add_widget(self.lbl_account)
        status_row.add_widget(self.lbl_status)
        
        # Thống kê nhanh
        self.lbl_stats = Label(
            text="Reactions lượt này: 0 | Tổng đã react: 0",
            font_size="13sp",
            color=COLOR_TEXT_MUTED,
            halign="left",
            valign="middle",
            size_hint_y=None,
            height="24dp",
        )
        self.lbl_stats.bind(size=self.lbl_stats.setter("text_size"))
        
        self.status_card.add_widget(status_row)
        self.status_card.add_widget(self.lbl_stats)
        self.add_widget(self.status_card)

        # --- 2. Action Controls: Nút Bật/Tắt to rõ ràng ---
        btn_box = BoxLayout(orientation="horizontal", size_hint_y=None, height="48dp", spacing="10dp")
        self.btn_toggle = StyledButton(
            text="BẮT ĐẦU CHẠY BOT",
            bg_color=COLOR_SUCCESS,
            font_size="16sp",
        )
        self.btn_toggle.bind(on_press=self.on_toggle_pressed)
        btn_box.add_widget(self.btn_toggle)
        self.add_widget(btn_box)

        # --- 3. Log Console Card ---
        log_header_box = BoxLayout(orientation="horizontal", size_hint_y=None, height="32dp")
        log_header_box.add_widget(SectionHeader(text="Nhật ký hoạt động (Live Logs)"))
        
        self.btn_clear_log = StyledButton(
            text="Xóa log",
            bg_color=get_color_from_hex("#4e5058"),
            size_hint_x=None,
            width="80dp",
            font_size="12sp",
        )
        self.btn_clear_log.bind(on_press=self.clear_logs)
        log_header_box.add_widget(self.btn_clear_log)
        self.add_widget(log_header_box)

        # Log Text Box
        self.log_input = TextInput(
            text="Sẵn sàng...\n",
            readonly=True,
            background_color=COLOR_INPUT_BG,
            foreground_color=get_color_from_hex("#dbdee1"),
            font_size="12sp",
            font_name="Roboto",
            padding=[8, 8, 8, 8],
        )
        self.add_widget(self.log_input)

        # Hẹn giờ cập nhật log và trạng thái định kỳ 1.0 giây
        self._last_log_size = 0
        Clock.schedule_interval(self.update_live_data, 1.0)
        self.refresh_account_info()

    def refresh_account_info(self):
        acc = self.app_controller.config_mgr.get_active_account()
        if acc:
            masked_token = f"{acc['token'][:6]}...{acc['token'][-4:]}" if len(acc.get("token", "")) > 10 else "***"
            self.lbl_account.text = f"Tài khoản: {acc.get('name', 'N/A')} ({masked_token})"
        else:
            self.lbl_account.text = "Tài khoản: Chưa chọn"

    def on_toggle_pressed(self, instance):
        is_running = self.app_controller.is_bot_running()
        if is_running:
            self.app_controller.stop_bot()
            self.set_ui_state(running=False)
        else:
            ok = self.app_controller.start_bot()
            if ok:
                self.set_ui_state(running=True)

    def set_ui_state(self, running: bool):
        if running:
            self.lbl_status.text = "● ĐANG CHẠY"
            self.lbl_status.color = COLOR_SUCCESS
            self.btn_toggle.text = "DỪNG HOẠT ĐỘNG"
            self.btn_toggle.set_bg_color(COLOR_DANGER)
        else:
            self.lbl_status.text = "● ĐÃ DỪNG"
            self.lbl_status.color = COLOR_DANGER
            self.btn_toggle.text = "BẮT ĐẦU CHẠY BOT"
            self.btn_toggle.set_bg_color(COLOR_SUCCESS)

    def update_live_data(self, dt):
        # 1. Kiểm tra trạng thái service/bot
        running = self.app_controller.is_bot_running()
        self.set_ui_state(running)

        # 2. Cập nhật thống kê
        stats = self.app_controller.get_current_stats()
        self.lbl_stats.text = f"Reactions lượt này: {stats.get('session_reacted', 0)} | Tổng kênh quét: {stats.get('total_channels', 0)}"

        # 3. Đọc log mới nếu file có thay đổi
        if LOG_FILE.exists():
            try:
                curr_size = LOG_FILE.stat().st_size
                if curr_size != self._last_log_size:
                    self._last_log_size = curr_size
                    with open(LOG_FILE, "r", encoding="utf-8") as f:
                        lines = f.readlines()
                        # Giữ tối đa 150 dòng cuối cùng để tiết kiệm RAM trên điện thoại
                        tail = lines[-150:]
                        self.log_input.text = "".join(tail)
                        self.log_input.cursor = (0, len(tail))
            except Exception:
                pass

    def clear_logs(self, instance):
        try:
            with open(LOG_FILE, "w", encoding="utf-8") as f:
                f.write("")
            self.log_input.text = "Đã làm sạch nhật ký.\n"
            self._last_log_size = 0
        except Exception:
            pass
