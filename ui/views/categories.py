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


class CategoriesView(BoxLayout):
    """Màn hình Quản lý Danh mục (Categories) và Danh sách Kênh loại trừ (Excluded Channels)."""

    def __init__(self, app_controller, **kwargs):
        super().__init__(**kwargs)
        self.app_controller = app_controller
        self.orientation = "vertical"
        self.spacing = "12dp"
        self.padding = ["12dp", "12dp", "12dp", "12dp"]

        # Tiêu đề và nút thêm mới
        header_box = BoxLayout(orientation="horizontal", size_hint_y=None, height="40dp")
        header_box.add_widget(SectionHeader(text="Danh mục Category quét"))

        btn_add = StyledButton(
            text="+ Thêm Category",
            bg_color=COLOR_PRIMARY,
            size_hint_x=None,
            width="140dp",
            font_size="13sp",
        )
        btn_add.bind(on_press=self.show_add_category_popup)
        header_box.add_widget(btn_add)
        self.add_widget(header_box)

        # Danh sách categories có cuộn
        self.scroll = ScrollView(size_hint=(1, 1))
        self.cat_list_layout = GridLayout(cols=1, spacing="10dp", size_hint_y=None)
        self.cat_list_layout.bind(minimum_height=self.cat_list_layout.setter("height"))
        self.scroll.add_widget(self.cat_list_layout)
        self.add_widget(self.scroll)

        self.refresh_categories()

    def refresh_categories(self):
        self.cat_list_layout.clear_widgets()
        categories = self.app_controller.config_mgr.get_categories()

        if not categories:
            empty_lbl = Label(
                text="Chưa cấu hình Category nào.\nBấm '+ Thêm Category' ở trên để thêm ID.",
                font_size="14sp",
                color=COLOR_TEXT_MUTED,
                size_hint_y=None,
                height="100dp",
                halign="center",
            )
            self.cat_list_layout.add_widget(empty_lbl)
            return

        for idx, cat in enumerate(categories):
            card = self._create_category_item(cat, idx)
            self.cat_list_layout.add_widget(card)

    def _create_category_item(self, cat, index):
        card = Card(
            orientation="vertical",
            size_hint_y=None,
            height="110dp",
            padding=["12dp", "10dp", "12dp", "10dp"],
            spacing="6dp",
        )

        top_row = BoxLayout(orientation="horizontal", size_hint_y=None, height="30dp")
        name = cat.get("name", f"Category #{index + 1}")
        cat_id = cat.get("id", "")
        lbl_title = Label(
            text=f"{name} (ID: {cat_id})",
            font_size="15sp",
            bold=True,
            color=COLOR_TEXT_PRIMARY,
            halign="left",
            valign="middle",
        )
        lbl_title.bind(size=lbl_title.setter("text_size"))
        top_row.add_widget(lbl_title)

        btn_edit = StyledButton(
            text="Sửa",
            bg_color=COLOR_PRIMARY,
            size_hint_x=None,
            width="55dp",
            font_size="12sp",
        )
        btn_edit.bind(on_press=lambda inst, i=index: self.show_edit_category_popup(i))
        top_row.add_widget(btn_edit)

        btn_delete = StyledButton(
            text="Xóa",
            bg_color=COLOR_DANGER,
            size_hint_x=None,
            width="55dp",
            font_size="12sp",
        )
        btn_delete.bind(on_press=lambda inst, i=index: self.delete_category(i))
        top_row.add_widget(btn_delete)
        card.add_widget(top_row)

        excluded = cat.get("excluded_channels", [])
        exc_str = ", ".join(str(cid) for cid in excluded) if excluded else "(Không loại trừ kênh nào)"
        lbl_exc = Label(
            text=f"Kênh loại trừ ({len(excluded)}): {exc_str}",
            font_size="12sp",
            color=COLOR_TEXT_MUTED,
            halign="left",
            valign="top",
        )
        lbl_exc.bind(size=lbl_exc.setter("text_size"))
        card.add_widget(lbl_exc)

        return card

    def delete_category(self, index):
        categories = self.app_controller.config_mgr.get_categories()
        if 0 <= index < len(categories):
            categories.pop(index)
            self.app_controller.config_mgr.save_categories(categories)
            self.refresh_categories()

    def show_add_category_popup(self, instance):
        self._show_cat_dialog("Thêm Category mới", None)

    def show_edit_category_popup(self, index):
        categories = self.app_controller.config_mgr.get_categories()
        if 0 <= index < len(categories):
            self._show_cat_dialog("Chỉnh sửa Category", index, categories[index])

    def _show_cat_dialog(self, title, index, cat_data=None):
        layout = BoxLayout(orientation="vertical", spacing="10dp", padding="14dp")

        lbl_name = Label(text="Tên Category:", size_hint_y=None, height="20dp", color=COLOR_TEXT_PRIMARY)
        input_name = StyledInput(
            text=cat_data.get("name", "") if cat_data else "",
            hint_text="Ví dụ: Kênh Chat Chính",
            size_hint_y=None,
            height="38dp",
        )

        lbl_id = Label(text="Category ID (Bắt buộc):", size_hint_y=None, height="20dp", color=COLOR_TEXT_PRIMARY)
        input_id = StyledInput(
            text=str(cat_data.get("id", "")) if cat_data else "",
            hint_text="Dán Category ID của Discord vào đây",
            size_hint_y=None,
            height="38dp",
        )

        lbl_exc = Label(
            text="Channel ID loại trừ (phân tách bằng dấu phẩy):",
            size_hint_y=None,
            height="20dp",
            color=COLOR_TEXT_PRIMARY,
        )
        exc_channels = cat_data.get("excluded_channels", []) if cat_data else []
        input_exc = StyledInput(
            text=", ".join(str(c) for c in exc_channels),
            hint_text="1229710841927045222, 1246901718143205437...",
            multiline=True,
            size_hint_y=None,
            height="70dp",
        )

        btn_box = BoxLayout(orientation="horizontal", spacing="10dp", size_hint_y=None, height="44dp")
        btn_cancel = StyledButton(text="Hủy", bg_color=get_color_from_hex("#4e5058"))
        btn_save = StyledButton(text="Lưu", bg_color=COLOR_SUCCESS)
        btn_box.add_widget(btn_cancel)
        btn_box.add_widget(btn_save)

        layout.add_widget(lbl_name)
        layout.add_widget(input_name)
        layout.add_widget(lbl_id)
        layout.add_widget(input_id)
        layout.add_widget(lbl_exc)
        layout.add_widget(input_exc)
        layout.add_widget(btn_box)

        popup = Popup(title=title, content=layout, size_hint=(0.92, 0.65), auto_dismiss=False)

        def on_save(inst):
            raw_id = input_id.text.strip()
            if not raw_id:
                return
            try:
                cat_id_int = int(raw_id)
            except ValueError:
                return

            # Xử lý danh sách excluded channels
            excluded_list = []
            for item in input_exc.text.replace("\n", ",").split(","):
                item_clean = item.strip()
                if item_clean.isdigit():
                    excluded_list.append(int(item_clean))

            name = input_name.text.strip() or f"Category {cat_id_int}"
            item_obj = {"id": cat_id_int, "name": name, "excluded_channels": excluded_list}

            categories = self.app_controller.config_mgr.get_categories()
            if index is not None and 0 <= index < len(categories):
                categories[index] = item_obj
            else:
                categories.append(item_obj)

            self.app_controller.config_mgr.save_categories(categories)
            self.refresh_categories()
            popup.dismiss()

        btn_cancel.bind(on_press=popup.dismiss)
        btn_save.bind(on_press=on_save)
        popup.open()
