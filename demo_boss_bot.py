import asyncio
import json
import os
import sys
import aiohttp

TOKEN = sys.argv[1].strip() if len(sys.argv) > 1 else os.environ.get("DISCORD_BOT_TOKEN", "").strip()

API_URL = "https://discord.com/api/v10"
GATEWAY_URL = "wss://gateway.discord.gg/?v=10&encoding=json"

# Kênh test mặc định trong server của bạn
DEFAULT_TEST_CHANNEL = "1511740284893921511" # Kênh 🧪-test

# Trạng thái trận Boss
boss_state = {
    "hp": 291665,
    "max_hp": 428430,
    "round": 1,
    "message_id": None,
    "channel_id": DEFAULT_TEST_CHANNEL
}

def log(msg):
    print(msg, flush=True)

def get_headers():
    return {
        "Authorization": f"Bot {TOKEN}",
        "Content-Type": "application/json",
        "User-Agent": "DiscordBot (DemoBossBot, 1.0)"
    }

def create_boss_payload(round_num, hp, max_hp):
    return {
        "content": "**🔥 [THÔNG BÁO TRẬN ĐÁNH BOSS DEMO]**",
        "embeds": [
            {
                "title": "⚔️ BOSS WAIFU: Cattleya Glory",
                "description": "“Khoảng lặng này không dành cho kẻ do dự.”\n🔥 **Cattleya Glory** đạt 100 Nộ và tung **Phá Vỡ Giới Hạn — Cuồng Nộ Diệt Ngũ!**\n*Tool Auto-Reactor sẽ tự động bấm nút đỏ bên dưới để tấn công!*",
                "color": 0xe74c3c, # Màu đỏ Danger
                "fields": [
                    {"name": "🩸 Máu Boss", "value": f"**{hp:,} / {max_hp:,}**", "inline": True},
                    {"name": "🔄 Hiệp đấu", "value": f"VÒNG **{round_num}** / 10", "inline": True},
                    {"name": "⏳ Thời gian", "value": "CÒN 06:00", "inline": True}
                ],
                "footer": {"text": "Bot Demo Test Đánh Boss • Sẵn sàng nhận click từ Auto Reactor"}
            }
        ],
        "components": [
            {
                "type": 1, # ActionRow
                "components": [
                    {
                        "type": 2, # Button
                        "style": 4, # Danger (Đỏ)
                        "label": "Đánh Boss",
                        "emoji": {"name": "⚔️"},
                        "custom_id": "attack_boss_demo",
                        "disabled": hp <= 0
                    },
                    {
                        "type": 2, # Button
                        "style": 2, # Secondary (Xám)
                        "label": "BXH sát thương",
                        "emoji": {"name": "🏆"},
                        "custom_id": "rank_boss_demo"
                    }
                ]
            }
        ]
    }

async def send_boss_message(session, channel_id):
    url = f"{API_URL}/channels/{channel_id}/messages"
    payload = create_boss_payload(boss_state["round"], boss_state["hp"], boss_state["max_hp"])
    async with session.post(url, headers=get_headers(), json=payload) as resp:
        if resp.status in (200, 201):
            data = await resp.json()
            boss_state["message_id"] = data["id"]
            boss_state["channel_id"] = channel_id
            log(f"\n[⚔️ BOSS SPAWN THÀNH CÔNG]")
            log(f"  👉 Kênh ID: {channel_id}")
            log(f"  👉 Tin nhắn ID: {data['id']}")
            log(f"  👉 Nút xuất hiện: '⚔️ Đánh Boss' (custom_id: attack_boss_demo)")
            log(f"👉 Hãy bật Tool Auto Reactor và cấu hình ID Kênh: {channel_id} để xem tool tự đánh!\n")
        else:
            err = await resp.text()
            log(f"[!] Lỗi gửi tin nhắn Boss: {resp.status} - {err}")

async def handle_interaction(session, interaction):
    data = interaction.get("data", {})
    custom_id = data.get("custom_id")
    user = interaction.get("member", {}).get("user") or interaction.get("user", {})
    user_name = user.get("username", "Người chơi")
    user_id = user.get("id", "0")

    inter_id = interaction.get("id")
    inter_token = interaction.get("token")
    callback_url = f"{API_URL}/interactions/{inter_id}/{inter_token}/callback"

    if custom_id == "attack_boss_demo":
        dmg = 15200
        boss_state["hp"] = max(0, boss_state["hp"] - dmg)
        boss_state["round"] = min(10, boss_state["round"] + 1)

        log("\n" + "=" * 60)
        log("🎯 [DEMO BOT: NHẬN ĐƯỢC CLICK NÚT ĐÁNH BOSS TỪ TOOL!]")
        log(f"  👤 Người bấm: {user_name} (ID: {user_id})")
        log(f"  ⚔️ Nút đã bấm: '⚔️ Đánh Boss' (custom_id: {custom_id})")
        log(f"  🩸 Sát thương gây ra: -{dmg:,} HP (Máu boss còn: {boss_state['hp']:,} / {boss_state['max_hp']:,})")
        log(f"  🔄 Tiến độ: Vòng {boss_state['round']} / 10")
        log("=" * 60 + "\n")

        # Phản hồi interaction cho Discord
        reply_payload = {
            "type": 4, # CHANNEL_MESSAGE_WITH_SOURCE
            "data": {
                "content": f"⚔️ **@{user_name}** đã tung đòn tấn công Boss thành công! Gây **{dmg:,}** sát thương! 🩸 Máu còn: **{boss_state['hp']:,}**"
            }
        }
        async with session.post(callback_url, headers=get_headers(), json=reply_payload) as resp:
            pass

        # Cập nhật lại tin nhắn Boss
        msg_id = boss_state.get("message_id")
        ch_id = boss_state.get("channel_id")
        if msg_id and ch_id:
            edit_url = f"{API_URL}/channels/{ch_id}/messages/{msg_id}"
            new_payload = create_boss_payload(boss_state["round"], boss_state["hp"], boss_state["max_hp"])
            async with session.patch(edit_url, headers=get_headers(), json=new_payload) as resp:
                pass

    elif custom_id == "rank_boss_demo":
        reply_payload = {
            "type": 4,
            "data": {
                "content": f"🏆 **BXH sát thương hiện tại:**\n1. @{user_name}: 35,400\n2. Darling: 24,612",
                "flags": 64 # Ephemeral
            }
        }
        async with session.post(callback_url, headers=get_headers(), json=reply_payload) as resp:
            pass

async def heartbeat(ws, interval):
    while True:
        await asyncio.sleep(interval)
        try:
            await ws.send_json({"op": 1, "d": None})
        except Exception:
            break

async def main():
    async with aiohttp.ClientSession() as session:
        # Lấy thông tin bot
        async with session.get(f"{API_URL}/users/@me", headers=get_headers()) as resp:
            if resp.status != 200:
                log(f"❌ Token bot không hợp lệ! HTTP {resp.status}")
                return
            bot_info = await resp.json()
            log("=" * 60)
            log(f"🤖 Bot Demo đã sẵn sàng: {bot_info['username']} (ID: {bot_info['id']})")
            log(f"🎯 Kênh test mặc định: #{DEFAULT_TEST_CHANNEL}")
            log("=" * 60)

        # Gửi luôn 1 tin nhắn boss ban đầu để sẵn sàng test
        await send_boss_message(session, DEFAULT_TEST_CHANNEL)

        # Kết nối Discord Gateway
        while True:
            try:
                async with session.ws_connect(GATEWAY_URL) as ws:
                    hb_task = None
                    async for msg in ws:
                        if msg.type == aiohttp.WSMsgType.TEXT:
                            data = json.loads(msg.data)
                            op = data.get("op")
                            t = data.get("t")
                            d = data.get("d")

                            if op == 10:
                                interval = d["heartbeat_interval"] / 1000.0
                                if hb_task:
                                    hb_task.cancel()
                                hb_task = asyncio.create_task(heartbeat(ws, interval))

                                identify_payload = {
                                    "op": 2,
                                    "d": {
                                        "token": TOKEN,
                                        "intents": 513, # GUILDS + GUILD_MESSAGES
                                        "properties": {
                                            "os": "windows",
                                            "browser": "demo_bot",
                                            "device": "demo_bot"
                                        }
                                    }
                                }
                                await ws.send_json(identify_payload)

                            elif t == "READY":
                                log("✅ [GATEWAY SẴN SÀNG] Bot Demo đang chạy nền và chờ Tool bấm nút!")

                            elif t == "INTERACTION_CREATE":
                                asyncio.create_task(handle_interaction(session, d))

            except Exception as e:
                log(f"[!] Mất kết nối Gateway, đang kết nối lại sau 3s: {e}")
                await asyncio.sleep(3)

if __name__ == "__main__":
    if not TOKEN:
        log("\n=== BOT DISCORD DEMO TEST ĐÁNH BOSS ===")
        TOKEN = input("Nhập Bot Token (hoặc chạy: python demo_boss_bot.py <TOKEN>): ").strip()
        if not TOKEN:
            log("❌ Chưa nhập Token!")
            sys.exit(1)
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        log("\nĐã dừng Bot Demo.")
