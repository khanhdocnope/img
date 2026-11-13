from kivy.uix.boxlayout import BoxLayout
from kivy.uix.gridlayout import GridLayout
from kivy.uix.scrollview import ScrollView
from kivy.uix.label import Label
from kivy.uix.popup import Popup
from kivy.utils import get_color_from_hex

from ui.components import (
    Card,
    StyledButton,
    StyledInput,
    SectionHeader,
    COLOR_PRIMARY,
    COLOR_SUCCESS,
    COLOR_DANGER,
    COLOR_TEXT_PRIMARY,
    COLOR_TEXT_MUTED,
)


class AccountsView(BoxLayout):
    """Màn hình Quản lý Đa tài khoản: Thêm mới, chọn tài khoản hoạt động, xóa tài khoản."""

    def __init__(self, app_controller, **kwargs):
        super().__init__(**kwargs)
        self.app_controller = app_controller
        self.orientation = "vertical"
        self.spacing = "12dp"
        self.padding = ["12dp", "12dp", "12dp", "12dp"]

        # Tiêu đề và nút thêm mới
        header_box = BoxLayout(orientation="horizontal", size_hint_y=None, height="40dp")
        header_box.add_widget(SectionHeader(text="Danh sách tài khoản Discord"))
        
        btn_add = StyledButton(
            text="+ Thêm tài khoản",
            bg_color=COLOR_PRIMARY,
            size_hint_x=None,
            width="140dp",
            font_size="13sp",
        )
        btn_add.bind(on_press=self.show_add_account_popup)
        header_box.add_widget(btn_add)
        self.add_widget(header_box)

        # Danh sách tài khoản có cuộn
        self.scroll = ScrollView(size_hint=(1, 1))
        self.account_list_layout = GridLayout(cols=1, spacing="10dp", size_hint_y=None)
        self.account_list_layout.bind(minimum_height=self.account_list_layout.setter("height"))
        self.scroll.add_widget(self.account_list_layout)
        self.add_widget(self.scroll)

        self.refresh_accounts()

    def refresh_accounts(self):
        self.account_list_layout.clear_widgets()
        accounts = self.app_controller.config_mgr.get_accounts()
        active_acc = self.app_controller.config_mgr.get_active_account()
        active_id = active_acc.get("id") if active_acc else None

        if not accounts:
            empty_lbl = Label(
                text="Chưa có tài khoản nào.\nBấm '+ Thêm tài khoản' ở trên để bắt đầu.",
                font_size="14sp",
                color=COLOR_TEXT_MUTED,
                size_hint_y=None,
                height="100dp",
                halign="center",
            )
            self.account_list_layout.add_widget(empty_lbl)
            return

        for acc in accounts:
            card = self._create_account_item(acc, is_active=(acc.get("id") == active_id))
            self.account_list_layout.add_widget(card)

    def _create_account_item(self, acc, is_active=False):
        card = Card(
            orientation="horizontal",
            size_hint_y=None,
            height="68dp",
            padding=["12dp", "8dp", "12dp", "8dp"],
            spacing="10dp",
        )

        # Cột thông tin tên & token rút gọn
        info_box = BoxLayout(orientation="vertical")
        status_tag = "[KÍCH HOẠT] " if is_active else ""
        lbl_name = Label(
            text=f"{status_tag}{acc.get('name', 'Không tên')}",
            font_size="15sp",
            bold=True,
            color=COLOR_SUCCESS if is_active else COLOR_TEXT_PRIMARY,
            halign="left",
            valign="middle",
        )
        lbl_name.bind(size=lbl_name.setter("text_size"))

        token = acc.get("token", "")
        masked_token = f"{token[:8]}...{token[-6:]}" if len(token) > 14 else "****************"
        lbl_token = Label(
            text=masked_token,
            font_size="12sp",
            color=COLOR_TEXT_MUTED,
            halign="left",
            valign="middle",
        )
        lbl_token.bind(size=lbl_token.setter("text_size"))

        info_box.add_widget(lbl_name)
        info_box.add_widget(lbl_token)
        card.add_widget(info_box)

        # Nút chọn kích hoạt nếu chưa kích hoạt
        if not is_active:
            btn_select = StyledButton(
                text="Chọn",
                bg_color=COLOR_PRIMARY,
                size_hint_x=None,
                width="70dp",
                font_size="13sp",
            )
            btn_select.bind(on_press=lambda inst, aid=acc["id"]: self.select_account(aid))
            card.add_widget(btn_select)

        # Nút Xóa
        btn_delete = StyledButton(
            text="Xóa",
            bg_color=COLOR_DANGER,
            size_hint_x=None,
            width="60dp",
            font_size="13sp",
        )
        btn_delete.bind(on_press=lambda inst, aid=acc["id"]: self.delete_account(aid))
        card.add_widget(btn_delete)

        return card

    def select_account(self, account_id):
        self.app_controller.config_mgr.set_active_account(account_id)
        self.refresh_accounts()
        if hasattr(self.app_controller, "dashboard_view"):
            self.app_controller.dashboard_view.refresh_account_info()

    def delete_account(self, account_id):
        self.app_controller.config_mgr.delete_account(account_id)
        self.refresh_accounts()
        if hasattr(self.app_controller, "dashboard_view"):
            self.app_controller.dashboard_view.refresh_account_info()

    def show_add_account_popup(self, instance):
        layout = BoxLayout(orientation="vertical", spacing="12dp", padding="16dp")
        
        lbl_name = Label(text="Tên gợi nhớ (ví dụ: Tài khoản 1):", size_hint_y=None, height="24dp", color=COLOR_TEXT_PRIMARY)
        input_name = StyledInput(hint_text="Nhập tên gợi nhớ", size_hint_y=None, height="40dp")
        
        lbl_token = Label(text="Discord User Token:", size_hint_y=None, height="24dp", color=COLOR_TEXT_PRIMARY)
        input_token = StyledInput(hint_text="Dán Discord token của bạn vào đây", password=True, size_hint_y=None, height="40dp")
        
        btn_box = BoxLayout(orientation="horizontal", spacing="10dp", size_hint_y=None, height="44dp")
        btn_cancel = StyledButton(text="Hủy", bg_color=get_color_from_hex("#4e5058"))
        btn_save = StyledButton(text="Lưu tài khoản", bg_color=COLOR_SUCCESS)
        
        btn_box.add_widget(btn_cancel)
        btn_box.add_widget(btn_save)
        
        layout.add_widget(lbl_name)
        layout.add_widget(input_name)
        layout.add_widget(lbl_token)
        layout.add_widget(input_token)
        layout.add_widget(btn_box)

        popup = Popup(
            title="Thêm tài khoản Discord mới",
            content=layout,
            size_hint=(0.9, 0.5),
            auto_dismiss=False,
        )

        def on_save(inst):
            name = input_name.text.strip() or "Tài khoản Discord"
            token = input_token.text.strip()
            if not token:
                return
            self.app_controller.config_mgr.add_or_update_account(name, token)
            self.refresh_accounts()
            if hasattr(self.app_controller, "dashboard_view"):
                self.app_controller.dashboard_view.refresh_account_info()
            popup.dismiss()

        btn_cancel.bind(on_press=popup.dismiss)
        btn_save.bind(on_press=on_save)
        popup.open()
