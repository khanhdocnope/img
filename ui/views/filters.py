from kivy.uix.boxlayout import BoxLayout
from kivy.uix.scrollview import ScrollView
from kivy.uix.label import Label
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


class FiltersView(BoxLayout):
    """Màn hình Bộ lọc Emoji: Whitelist (Chỉ react emoji này) & Blacklist (Cấm react emoji này)."""

    def __init__(self, app_controller, **kwargs):
        super().__init__(**kwargs)
        self.app_controller = app_controller
        self.orientation = "vertical"
        self.spacing = "12dp"
        self.padding = ["12dp", "12dp", "12dp", "12dp"]

        self.add_widget(SectionHeader(text="Bộ lọc cảm xúc (Emoji Whitelist / Blacklist)"))

        scroll = ScrollView(size_hint=(1, 1))
        content = BoxLayout(orientation="vertical", spacing="14dp", size_hint_y=None)
        content.bind(minimum_height=content.setter("height"))

        filters = self.app_controller.config_mgr.get_emoji_filters()

        # 1. Emoji Whitelist
        card_white = Card(orientation="vertical", size_hint_y=None, height="140dp", padding="10dp", spacing="4dp")
        lbl_white = Label(
            text="Danh sách cho phép (Whitelist - Bỏ trống để react tất cả):",
            font_size="14sp",
            bold=True,
            color=COLOR_TEXT_PRIMARY,
            halign="left",
            size_hint_y=None,
            height="24dp",
        )
        lbl_white.bind(size=lbl_white.setter("text_size"))
        
        lbl_white_hint = Label(
            text="Nhập các emoji phân tách bằng dấu phẩy, ví dụ: 👍, ❤️, 🎉",
            font_size="12sp",
            color=COLOR_TEXT_MUTED,
            halign="left",
            size_hint_y=None,
            height="18dp",
        )
        lbl_white_hint.bind(size=lbl_white_hint.setter("text_size"))

        self.input_white = StyledInput(
            text=", ".join(filters.get("whitelist", [])),
            hint_text="👍, ❤️, 🚀",
            multiline=True,
            size_hint_y=None,
            height="65dp",
        )
        card_white.add_widget(lbl_white)
        card_white.add_widget(lbl_white_hint)
        card_white.add_widget(self.input_white)
        content.add_widget(card_white)

        # 2. Emoji Blacklist
        card_black = Card(orientation="vertical", size_hint_y=None, height="140dp", padding="10dp", spacing="4dp")
        lbl_black = Label(
            text="Danh sách cấm (Blacklist - Bỏ qua không bao giờ react):",
            font_size="14sp",
            bold=True,
            color=COLOR_TEXT_PRIMARY,
            halign="left",
            size_hint_y=None,
            height="24dp",
        )
        lbl_black.bind(size=lbl_black.setter("text_size"))

        lbl_black_hint = Label(
            text="Nhập các emoji muốn bỏ qua phân tách bằng dấu phẩy, ví dụ: 👎, 💩",
            font_size="12sp",
            color=COLOR_TEXT_MUTED,
            halign="left",
            size_hint_y=None,
            height="18dp",
        )
        lbl_black_hint.bind(size=lbl_black_hint.setter("text_size"))

        self.input_black = StyledInput(
            text=", ".join(filters.get("blacklist", [])),
            hint_text="👎, 😡, 💩",
            multiline=True,
            size_hint_y=None,
            height="65dp",
        )
        card_black.add_widget(lbl_black)
        card_black.add_widget(lbl_black_hint)
        card_black.add_widget(self.input_black)
        content.add_widget(card_black)

        scroll.add_widget(content)
        self.add_widget(scroll)

        # Nút Lưu
        self.btn_save = StyledButton(
            text="LƯU BỘ LỌC EMOJI",
            bg_color=COLOR_PRIMARY,
            size_hint_y=None,
            height="48dp",
        )
        self.btn_save.bind(on_press=self.save_filters)
        self.add_widget(self.btn_save)

    def save_filters(self, instance):
        whitelist = [e.strip() for e in self.input_white.text.split(",") if e.strip()]
        blacklist = [e.strip() for e in self.input_black.text.split(",") if e.strip()]

        self.app_controller.config_mgr.update_emoji_filters(whitelist, blacklist)
        self.btn_save.text = "ĐÃ LƯU BỘ LỌC THÀNH CÔNG!"
        self.btn_save.set_bg_color(COLOR_SUCCESS)

        from kivy.clock import Clock
        Clock.schedule_once(self._restore_button, 1.5)

    def _restore_button(self, dt):
        self.btn_save.text = "LƯU BỘ LỌC EMOJI"
        self.btn_save.set_bg_color(COLOR_PRIMARY)
