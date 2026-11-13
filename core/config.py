import json
from pathlib import Path
from typing import Dict, List, Any, Optional
import os

BASE_DIR = Path(__file__).resolve().parent.parent
DATA_DIR = BASE_DIR / "data"
CONFIG_FILE = DATA_DIR / "config.json"
PROGRESS_FILE = DATA_DIR / "progress.json"
LOG_FILE = DATA_DIR / "bot_log.txt"

# Đảm bảo thư mục data luôn tồn tại
DATA_DIR.mkdir(parents=True, exist_ok=True)

DEFAULT_CONFIG: Dict[str, Any] = {
    "accounts": [],
    "active_account_id": None,
    "bot_settings": {
        "delay": 1.0,
        "max_messages_per_channel": None,
        "skip_bot_messages": True,
        "full_scan": True,
        "concurrent_channels": 4,
    },
    "categories": [
        {
            "id": 1281146625435369472,
            "name": "Mặc định",
            "excluded_channels": [
                1229710841927045222,
                1246901718143205437,
                1516859270140792842,
                1516859246476787933,
                1517099249257414777,
                1196522451857842287,
                1232731962834161775,
                1437860347032895518,
                1390315019006251008,
                1281171567513960461,
                1357284374684500159,
            ],
        }
    ],
    "emoji_whitelist": [],
    "emoji_blacklist": [],
}


class ConfigManager:
    """Quản lý đọc/ghi cấu hình đa tài khoản và thiết lập bot an toàn."""

    def __init__(self, config_path: Path = CONFIG_FILE, progress_path: Path = PROGRESS_FILE):
        self.config_path = config_path
        self.progress_path = progress_path
        self.config = self.load_config()

    def load_config(self) -> Dict[str, Any]:
        if not self.config_path.exists():
            self.save_config(DEFAULT_CONFIG)
            return json.loads(json.dumps(DEFAULT_CONFIG))

        try:
            with open(self.config_path, "r", encoding="utf-8") as f:
                data = json.load(f)
                # Đảm bảo các trường mặc định nếu file cũ thiếu
                merged = json.loads(json.dumps(DEFAULT_CONFIG))
                merged.update(data)
                return merged
        except Exception as e:
            print(f"[ConfigManager] Lỗi đọc config: {e}. Sử dụng cấu hình mặc định.")
            return json.loads(json.dumps(DEFAULT_CONFIG))

    def save_config(self, data: Optional[Dict[str, Any]] = None) -> bool:
        if data is not None:
            self.config = data
        try:
            temp_path = self.config_path.with_suffix(".tmp")
            with open(temp_path, "w", encoding="utf-8") as f:
                json.dump(self.config, f, indent=2, ensure_ascii=False)
            temp_path.replace(self.config_path)
            return True
        except Exception as e:
            print(f"[ConfigManager] Lỗi lưu config: {e}")
            return False

    # --- Quản lý Tài khoản (Accounts) ---
    def get_accounts(self) -> List[Dict[str, Any]]:
        return self.config.get("accounts", [])

    def get_active_account(self) -> Optional[Dict[str, Any]]:
        active_id = self.config.get("active_account_id")
        accounts = self.get_accounts()
        if not accounts:
            return None
        for acc in accounts:
            if acc.get("id") == active_id:
                return acc
        return accounts[0] if accounts else None

    def set_active_account(self, account_id: str) -> bool:
        self.config["active_account_id"] = account_id
        return self.save_config()

    def add_or_update_account(self, name: str, token: str, account_id: Optional[str] = None) -> str:
        accounts = self.get_accounts()
        if not account_id:
            import uuid
            account_id = f"acc_{uuid.uuid4().hex[:8]}"
            accounts.append({"id": account_id, "name": name, "token": token})
            if not self.config.get("active_account_id"):
                self.config["active_account_id"] = account_id
        else:
            updated = False
            for acc in accounts:
                if acc.get("id") == account_id:
                    acc["name"] = name
                    acc["token"] = token
                    updated = True
                    break
            if not updated:
                accounts.append({"id": account_id, "name": name, "token": token})

        self.config["accounts"] = accounts
        self.save_config()
        return account_id

    def delete_account(self, account_id: str) -> bool:
        accounts = [acc for acc in self.get_accounts() if acc.get("id") != account_id]
        self.config["accounts"] = accounts
        if self.config.get("active_account_id") == account_id:
            self.config["active_account_id"] = accounts[0]["id"] if accounts else None
        return self.save_config()

    # --- Quản lý Cài đặt Bot ---
    def get_bot_settings(self) -> Dict[str, Any]:
        return self.config.get("bot_settings", DEFAULT_CONFIG["bot_settings"])

    def update_bot_settings(self, **kwargs) -> bool:
        settings = self.get_bot_settings()
        for k, v in kwargs.items():
            if k in settings:
                settings[k] = v
        self.config["bot_settings"] = settings
        return self.save_config()

    # --- Quản lý Categories & Channels ---
    def get_categories(self) -> List[Dict[str, Any]]:
        return self.config.get("categories", [])

    def save_categories(self, categories: List[Dict[str, Any]]) -> bool:
        self.config["categories"] = categories
        return self.save_config()

    # --- Quản lý Emoji Filters ---
    def get_emoji_filters(self) -> Dict[str, List[str]]:
        return {
            "whitelist": self.config.get("emoji_whitelist", []),
            "blacklist": self.config.get("emoji_blacklist", []),
        }

    def update_emoji_filters(self, whitelist: List[str], blacklist: List[str]) -> bool:
        self.config["emoji_whitelist"] = whitelist
        self.config["emoji_blacklist"] = blacklist
        return self.save_config()

    # --- Quản lý Tiến trình (Progress) ---
    def load_progress(self) -> Dict[str, Any]:
        if not self.progress_path.exists():
            return {}
        try:
            with open(self.progress_path, "r", encoding="utf-8") as f:
                return json.load(f)
        except Exception:
            return {}

    def save_progress(self, data: Dict[str, Any]) -> bool:
        try:
            temp_path = self.progress_path.with_suffix(".tmp")
            with open(temp_path, "w", encoding="utf-8") as f:
                json.dump(data, f, indent=2, ensure_ascii=False)
            temp_path.replace(self.progress_path)
            return True
        except Exception as e:
            print(f"[ConfigManager] Lỗi lưu progress: {e}")
            return False

    def reset_progress(self) -> bool:
        try:
            if self.progress_path.exists():
                self.progress_path.unlink()
            return True
        except Exception as e:
            print(f"[ConfigManager] Lỗi xóa progress: {e}")
            return False
