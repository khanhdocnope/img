from kivy.uix.boxlayout import BoxLayout
from kivy.uix.scrollview import ScrollView
from kivy.uix.label import Label
from kivy.uix.switch import Switch
from kivy.utils import get_color_from_hex

from ui.components import (
    Card,
    StyledButton,
    StyledInput,
    SectionHeader,
    COLOR_PRIMARY,
    COLOR_SUCCESS,
    COLOR_TEXT_PRIMARY,
    COLOR_TEXT_MUTED,
)


class SettingsView(BoxLayout):
    """Màn hình Cài đặt Bot: Delay, Số luồng (Concurrency), Skip bots, Full scan."""

    def __init__(self, app_controller, **kwargs):
        super().__init__(**kwargs)
        self.app_controller = app_controller
        self.orientation = "vertical"
        self.spacing = "12dp"
        self.padding = ["12dp", "12dp", "12dp", "12dp"]

        self.add_widget(SectionHeader(text="Thiết lập tham số hoạt động của Bot"))

        scroll = ScrollView(size_hint=(1, 1))
        content = BoxLayout(orientation="vertical", spacing="14dp", size_hint_y=None)
        content.bind(minimum_height=content.setter("height"))

        settings = self.app_controller.config_mgr.get_bot_settings()

        # 1. Delay giữa các reactions
        card_delay = Card(orientation="vertical", size_hint_y=None, height="84dp", padding="10dp", spacing="4dp")
        lbl_delay = Label(
            text="Thời gian chờ giữa các lần thả cảm xúc (Delay - Giây):",
            font_size="14sp",
            bold=True,
            color=COLOR_TEXT_PRIMARY,
            halign="left",
            size_hint_y=None,
            height="24dp",
        )
        lbl_delay.bind(size=lbl_delay.setter("text_size"))
        self.input_delay = StyledInput(
            text=str(settings.get("delay", 1.0)),
            hint_text="Mặc định: 1.0 (Khuyến nghị >= 1.0 để tránh rate limit)",
            size_hint_y=None,
            height="40dp",
        )
        card_delay.add_widget(lbl_delay)
        card_delay.add_widget(self.input_delay)
        content.add_widget(card_delay)

        # 2. Số luồng quét đồng thời (Concurrent Channels)
        card_conc = Card(orientation="vertical", size_hint_y=None, height="84dp", padding="10dp", spacing="4dp")
        lbl_conc = Label(
            text="Số kênh xử lý đồng thời (Concurrent Channels):",
            font_size="14sp",
            bold=True,
            color=COLOR_TEXT_PRIMARY,
            halign="left",
            size_hint_y=None,
            height="24dp",
        )
        lbl_conc.bind(size=lbl_conc.setter("text_size"))
        self.input_conc = StyledInput(
            text=str(settings.get("concurrent_channels", 4)),
            hint_text="Mặc định: 4",
            size_hint_y=None,
            height="40dp",
        )
        card_conc.add_widget(lbl_conc)
        card_conc.add_widget(self.input_conc)
        content.add_widget(card_conc)

        # 3. Bỏ qua tin nhắn của Bot (Skip Bot Messages)
        card_skip = Card(orientation="horizontal", size_hint_y=None, height="60dp", padding="10dp")
        lbl_skip = Label(
            text="Bỏ qua tin nhắn từ Bot (Skip Bot Messages):",
            font_size="14sp",
            bold=True,
            color=COLOR_TEXT_PRIMARY,
            halign="left",
            valign="middle",
        )
        lbl_skip.bind(size=lbl_skip.setter("text_size"))
        self.switch_skip = Switch(active=settings.get("skip_bot_messages", True), size_hint_x=None, width="70dp")
        card_skip.add_widget(lbl_skip)
        card_skip.add_widget(self.switch_skip)
        content.add_widget(card_skip)

        # 4. Quét toàn bộ lịch sử (Full Scan)
        card_scan = Card(orientation="horizontal", size_hint_y=None, height="60dp", padding="10dp")
        lbl_scan = Label(
            text="Quét toàn bộ tin nhắn kênh (Full Scan):",
            font_size="14sp",
            bold=True,
            color=COLOR_TEXT_PRIMARY,
            halign="left",
            valign="middle",
        )
        lbl_scan.bind(size=lbl_scan.setter("text_size"))
        self.switch_scan = Switch(active=settings.get("full_scan", True), size_hint_x=None, width="70dp")
        card_scan.add_widget(lbl_scan)
        card_scan.add_widget(self.switch_scan)
        content.add_widget(card_scan)

        # 5. Nút Đặt lại tiến trình quét (Reset Progress)
        card_reset = Card(orientation="horizontal", size_hint_y=None, height="60dp", padding="10dp")
        lbl_reset = Label(
            text="Xóa tiến trình đã lưu (để quét lại từ đầu):",
            font_size="13sp",
            color=COLOR_TEXT_MUTED,
            halign="left",
            valign="middle",
        )
        lbl_reset.bind(size=lbl_reset.setter("text_size"))
        btn_reset = StyledButton(
            text="Đặt lại tiến trình",
            bg_color=get_color_from_hex("#da373c"),
            size_hint_x=None,
            width="140dp",
            font_size="12sp",
        )
        btn_reset.bind(on_press=self.on_reset_progress)
        card_reset.add_widget(lbl_reset)
        card_reset.add_widget(btn_reset)
        content.add_widget(card_reset)

        scroll.add_widget(content)
        self.add_widget(scroll)

        # Nút Lưu cấu hình
        self.btn_save = StyledButton(
            text="LƯU CÀI ĐẶT",
            bg_color=COLOR_PRIMARY,
            size_hint_y=None,
            height="48dp",
        )
        self.btn_save.bind(on_press=self.save_settings)
        self.add_widget(self.btn_save)

    def save_settings(self, instance):
        try:
            delay = float(self.input_delay.text.strip())
        except ValueError:
            delay = 1.0

        try:
            conc = int(self.input_conc.text.strip())
        except ValueError:
            conc = 4

        self.app_controller.config_mgr.update_bot_settings(
            delay=delay,
            concurrent_channels=conc,
            skip_bot_messages=self.switch_skip.active,
            full_scan=self.switch_scan.active,
        )
        self.btn_save.text = "ĐÃ LƯU THÀNH CÔNG!"
        self.btn_save.set_bg_color(COLOR_SUCCESS)

        from kivy.clock import Clock
        Clock.schedule_once(self._restore_save_button, 1.5)

    def _restore_save_button(self, dt):
        self.btn_save.text = "LƯU CÀI ĐẶT"
        self.btn_save.set_bg_color(COLOR_PRIMARY)

    def on_reset_progress(self, instance):
        self.app_controller.config_mgr.reset_progress()
        instance.text = "Đã đặt lại!"
        from kivy.clock import Clock
        Clock.schedule_once(lambda dt: setattr(instance, "text", "Đặt lại tiến trình"), 1.5)
