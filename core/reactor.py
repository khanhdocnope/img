import asyncio
import discord
from discord.ext import commands
from datetime import datetime
from pathlib import Path
from typing import Optional, Callable, Dict, Any, List

class DiscordReactorEngine:
    """
    Engine xử lý Discord Auto-Reactor tối ưu cho thiết bị di động.
    Hỗ trợ ngắt an toàn (graceful stop), kiểm soát rate-limit (429) lũy tiến,
    và callback báo cáo trạng thái / log trực tiếp về giao diện.
    """

    def __init__(
        self,
        config: Dict[str, Any],
        progress: Dict[str, Any],
        on_log: Optional[Callable[[str], None]] = None,
        on_status: Optional[Callable[[str, int], None]] = None,
        save_progress_fn: Optional[Callable[[Dict[str, Any]], None]] = None,
    ):
        self.config = config
        self.progress = progress
        self.on_log = on_log or (lambda msg: print(msg))
        self.on_status = on_status or (lambda status, count: None)
        self.save_progress_fn = save_progress_fn or (lambda p: None)

        self.running = False
        self.bot: Optional[commands.Bot] = None
        self.total_reacted_session = 0
        self.blocked_channels = set()
        self.clicked_buttons = {}  # click_key -> float(timestamp)
        self._stop_event = asyncio.Event()

    def log(self, msg: str):
        timestamp = datetime.now().strftime("%H:%M:%S")
        formatted = f"[{timestamp}] {msg}"
        self.on_log(formatted)

    def is_emoji_allowed(self, emoji) -> bool:
        emoji_str = str(emoji)
        whitelist = self.config.get("emoji_whitelist", [])
        blacklist = self.config.get("emoji_blacklist", [])

        if whitelist and emoji_str not in whitelist:
            return False
        if emoji_str in blacklist:
            return False
        return True

    async def add_reaction_with_retry(self, message, emoji) -> bool:
        settings = self.config.get("bot_settings", {})
        delay = float(settings.get("delay", 1.0))

        while self.running and not self._stop_event.is_set():
            try:
                await message.add_reaction(emoji)
                return True
            except discord.HTTPException as e:
                if e.status == 429:
                    retry_after = getattr(e, "retry_after", None)
                    if retry_after is None:
                        try:
                            retry_after = e.response.json().get("retry_after", 2.0) if hasattr(e, "response") else 2.0
                        except Exception:
                            retry_after = 2.0
                    try:
                        retry_after = float(retry_after) + 0.5
                    except Exception:
                        retry_after = 2.0
                    self.log(f"  [429] Rate limited, tạm dừng {retry_after:.2f}s...")
                    await asyncio.sleep(retry_after)
                    continue
                elif e.status == 403:
                    return False
                else:
                    err_txt = getattr(e, "text", str(e))
                    self.log(f"  [LỖI {e.status}] {err_txt[:100]}")
                    return False
            except Exception as e:
                self.log(f"  [LỖI] {e}")
                return False
        return False

    async def process_channel(self, channel) -> int:
        if not self.running or self._stop_event.is_set():
            return 0

        if getattr(channel, "id", None) in self.blocked_channels:
            return 0

        key = str(channel.id)
        ch_progress = self.progress.get(key, {})
        last_id = ch_progress.get("last_reacted_id")
        reacted = ch_progress.get("total_reacted", 0)

        settings = self.config.get("bot_settings", {})
        full_scan = settings.get("full_scan", True)
        max_msgs = settings.get("max_messages_per_channel", None)
        skip_bots = settings.get("skip_bot_messages", True)
        delay = float(settings.get("delay", 1.0))
        auto_button = settings.get("auto_button_enabled", False)
        btn_label_kw = settings.get("button_label_keyword", "Đánh Boss")
        btn_emoji_kw = settings.get("button_emoji_keyword", "⚔️")

        if full_scan:
            kwargs = {"limit": None}
        else:
            kwargs = {"limit": max_msgs}
            if last_id:
                try:
                    kwargs["after"] = discord.Object(id=int(last_id))
                except Exception:
                    pass

        new_reacted = 0
        last_reacted_id = None

        try:
            async for message in channel.history(**kwargs):
                if not self.running or self._stop_event.is_set():
                    break

                # 1. Tự động click nút Boss trên tin nhắn bot
                if auto_button:
                    comps = getattr(message, "components", []) or []
                    for comp in comps:
                        children = getattr(comp, "children", []) or getattr(comp, "components", []) or [comp]
                        for child in children:
                            custom_id = getattr(child, "custom_id", None) or getattr(child, "id", "")
                            click_key = f"{message.id}:{custom_id}"
                            now_t = time.time()
                            if now_t - self.clicked_buttons.get(click_key, 0) < 8.0:
                                continue

                            label = getattr(child, "label", "") or ""
                            emoji = getattr(child, "emoji", None)
                            emoji_name = getattr(emoji, "name", "") if emoji else ""

                            label_match = bool(btn_label_kw and (btn_label_kw.lower() in label.lower()))
                            emoji_match = bool(btn_emoji_kw and (btn_emoji_kw in emoji_name))

                            if label_match or emoji_match:
                                self.clicked_buttons[click_key] = now_t
                                click_fn = getattr(child, "click", None)
                                if callable(click_fn):
                                    try:
                                        await click_fn()
                                        self.log(f"  [⚔️ Đánh Boss] Đã bấm nút '{label or emoji_name}' tại #{channel.name}")
                                        await asyncio.sleep(0.5)
                                    except Exception as be:
                                        self.log(f"  [⚔️ Đánh Boss] Lỗi nhấn nút: {be}")

                if skip_bots and message.author.bot:
                    continue
                if not message.reactions:
                    continue

                for reaction in message.reactions:
                    if not self.running or self._stop_event.is_set():
                        break
                    if reaction.me:
                        continue
                    if not self.is_emoji_allowed(reaction.emoji):
                        continue

                    ok = await self.add_reaction_with_retry(message, reaction.emoji)
                    if ok:
                        new_reacted += 1
                        reacted += 1
                        self.total_reacted_session += 1
                        last_reacted_id = message.id
                        self.on_status("running", self.total_reacted_session)
                        self.log(f"  [+{reaction.emoji}] tin nhắn {message.id} (#{channel.name})")

                    await asyncio.sleep(delay)
        except discord.Forbidden:
            self.blocked_channels.add(channel.id)
            self.log(f"  [!] Không có quyền đọc: #{channel.name} (403), tự động bỏ qua kênh này.")
        except Exception as e:
            self.log(f"  [!] Lỗi kênh #{channel.name}: {e}")

        if last_reacted_id:
            self.progress[key] = {
                "name": channel.name,
                "last_reacted_id": str(last_reacted_id),
                "total_reacted": reacted,
            }
            self.save_progress_fn(self.progress)

        return new_reacted

    async def process_category(self, cat_cfg: Dict[str, Any], sem: asyncio.Semaphore) -> int:
        if not self.running or self._stop_event.is_set():
            return 0

        cat_id = cat_cfg.get("id")
        category = self.bot.get_channel(int(cat_id)) if self.bot else None
        if not category:
            self.log(f"[!] Không tìm thấy Category ID: {cat_id}")
            return 0

        excluded = set(cat_cfg.get("excluded_channels", []))
        channels = [
            ch for ch in category.channels
            if isinstance(ch, discord.TextChannel) and ch.id not in excluded
        ]

        if not channels:
            self.log(f"[{cat_id}] Không có kênh văn bản nào cần xử lý")
            return 0

        concurrency = self.config.get("bot_settings", {}).get("concurrent_channels", 4)
        self.log(f"[{category.name}] Xử lý {len(channels)} kênh (đồng thời {concurrency} luồng)...")

        async def worker(ch):
            async with sem:
                if not self.running or self._stop_event.is_set():
                    return 0
                return await self.process_channel(ch)

        results = await asyncio.gather(*[worker(ch) for ch in channels], return_exceptions=True)
        return sum(r for r in results if isinstance(r, int))

    async def run_loop(self):
        concurrency = self.config.get("bot_settings", {}).get("concurrent_channels", 4)
        sem = asyncio.Semaphore(concurrency)

        while self.running and not self._stop_event.is_set():
            categories = self.config.get("categories", [])
            if not categories:
                self.log("Chưa cấu hình danh mục (CATEGORIES). Chờ 30s...")
                await asyncio.sleep(30)
                continue

            total_round = 0
            for cat in categories:
                if not self.running or self._stop_event.is_set():
                    break
                total_round += await self.process_category(cat, sem)

            self.save_progress_fn(self.progress)
            self.log(f"Hoàn thành lượt quét! Thêm {total_round} reaction. Nghỉ 10s trước lượt mới...")

            # Cho phép hủy sleep ngay lập tức khi stop
            try:
                await asyncio.wait_for(self._stop_event.wait(), timeout=10.0)
                break
            except asyncio.TimeoutError:
                pass

    def stop(self):
        """Yêu cầu dừng bot an toàn."""
        self.running = False
        self._stop_event.set()
        if self.bot and not self.bot.is_closed():
            asyncio.create_task(self.bot.close())
        self.on_status("stopped", self.total_reacted_session)
        self.log("Bot đã được dừng an toàn.")

    async def start(self, token: str):
        """Khởi chạy Discord selfbot."""
        self.running = True
        self._stop_event.clear()
        self.on_status("starting", self.total_reacted_session)

        self.bot = commands.Bot(command_prefix="!", self_bot=True)

        @self.bot.event
        async def on_ready():
            self.log(f"Đăng nhập thành công: {self.bot.user} (ID: {self.bot.user.id})")
            self.on_status("running", self.total_reacted_session)
            asyncio.create_task(self.run_loop())

        @self.bot.event
        async def on_resumed():
            self.log("Kết nối mạng đã được khôi phục (Gateway Resumed)")

        try:
            self.log("Đang kết nối tới máy chủ Discord...")
            await self.bot.start(token)
        except discord.LoginFailure:
            self.log("[LỖI] Token không hợp lệ hoặc đã hết hạn!")
            self.stop()
        except Exception as e:
            self.log(f"[LỖI KẾT NỐI] {e}")
            self.stop()
