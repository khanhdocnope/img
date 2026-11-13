from kivy.uix.boxlayout import BoxLayout
from kivy.uix.button import Button
from kivy.uix.textinput import TextInput
from kivy.uix.label import Label
from kivy.graphics import Color, RoundedRectangle
from kivy.utils import get_color_from_hex

# Bảng màu phong cách Discord Dark Mode
COLOR_BG_DARK = get_color_from_hex("#1e1f22")
COLOR_CARD = get_color_from_hex("#2b2d31")
COLOR_INPUT_BG = get_color_from_hex("#111214")
COLOR_PRIMARY = get_color_from_hex("#5865f2")
COLOR_SUCCESS = get_color_from_hex("#23a55a")
COLOR_DANGER = get_color_from_hex("#da373c")
COLOR_TEXT_PRIMARY = get_color_from_hex("#f2f3f5")
COLOR_TEXT_MUTED = get_color_from_hex("#949ba4")
COLOR_BORDER = get_color_from_hex("#35373c")


class Card(BoxLayout):
    """Container hình hộp có bo góc và màu nền tối phong cách thẻ hiện đại."""

    def __init__(self, bg_color=COLOR_CARD, radius=[12, 12, 12, 12], **kwargs):
        super().__init__(**kwargs)
        self.bg_color = bg_color
        self.radius = radius
        with self.canvas.before:
            self._color_instr = Color(*self.bg_color)
            self._rect = RoundedRectangle(pos=self.pos, size=self.size, radius=self.radius)
        self.bind(pos=self._update_rect, size=self._update_rect)

    def _update_rect(self, *args):
        self._rect.pos = self.pos
        self._rect.size = self.size


class StyledButton(Button):
    """Nút bấm có bo góc mượt mà và hỗ trợ màu chủ đề."""

    def __init__(self, bg_color=COLOR_PRIMARY, radius=[10, 10, 10, 10], **kwargs):
        super().__init__(**kwargs)
        self.background_color = (0, 0, 0, 0)  # Tắt nền mặc định của Kivy
        self.background_normal = ""
        self.bg_color = bg_color
        self.radius = radius
        self.color = COLOR_TEXT_PRIMARY
        self.font_size = "15sp"
        self.bold = True
        with self.canvas.before:
            self._color_instr = Color(*self.bg_color)
            self._rect = RoundedRectangle(pos=self.pos, size=self.size, radius=self.radius)
        self.bind(pos=self._update_rect, size=self._update_rect)

    def set_bg_color(self, new_color):
        self.bg_color = new_color
        self._color_instr.rgba = new_color

    def _update_rect(self, *args):
        self._rect.pos = self.pos
        self._rect.size = self.size


class StyledInput(TextInput):
    """Ô nhập dữ liệu tối màu bo góc, hỗ trợ font rõ nét và con trỏ nổi bật."""

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self.background_color = COLOR_INPUT_BG
        self.foreground_color = COLOR_TEXT_PRIMARY
        self.cursor_color = COLOR_PRIMARY
        self.font_size = "14sp"
        self.padding = [12, 10, 12, 10]
        self.multiline = kwargs.get("multiline", False)


class SectionHeader(Label):
    """Tiêu đề phân mục với font đậm nổi bật."""

    def __init__(self, text="", **kwargs):
        super().__init__(**kwargs)
        self.text = text
        self.font_size = "16sp"
        self.bold = True
        self.color = COLOR_TEXT_PRIMARY
        self.size_hint_y = None
        self.height = "32dp"
        self.halign = "left"
        self.valign = "middle"
        self.bind(size=self.setter("text_size"))
