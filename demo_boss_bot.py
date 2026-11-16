import discord
from discord.ext import commands
import sys

# Khởi tạo Bot với tiền tố !
intents = discord.Intents.default()
intents.message_content = True
bot = commands.Bot(command_prefix="!", intents=intents)

class BossBattleView(discord.ui.View):
    def __init__(self, round_num=1, hp=291665, max_hp=428430):
        super().__init__(timeout=None) # Không timeout để nút luôn bấm được
        self.round_num = round_num
        self.hp = hp
        self.max_hp = max_hp

    @discord.ui.button(
        label="Đánh Boss",
        style=discord.ButtonStyle.danger, # Nút đỏ Danger giống hệt game
        emoji="⚔️",
        custom_id="attack_boss_demo"
    )
    async def attack_button(self, interaction: discord.Interaction, button: discord.ui.Button):
        # In log ra terminal để người dùng kiểm chứng
        user = interaction.user
        print(f"\n[🎯 DEMO BOT NHẬN CLICK THÀNH CÔNG!]")
        print(f"  👉 Người chơi: {user.name}#{user.discriminator} (ID: {user.id})")
        print(f"  👉 Đã bấm nút: '⚔️ Đánh Boss' (custom_id: {button.custom_id})")
        print(f"  👉 Kênh: #{interaction.channel.name} (ID: {interaction.channel_id})\n")

        # Trừ máu boss và tăng vòng
        dmg = 15200
        self.hp = max(0, self.hp - dmg)
        self.round_num = min(10, self.round_num + 1)

        # Phản hồi interaction cho Discord
        msg = f"⚔️ **{user.mention}** đã tung đòn tấn công Boss gây **{dmg:,}** sát thương!"
        if self.hp <= 0:
            msg += "\n🎉 **Boss đã bị tiêu diệt hoàn toàn!**"
            button.disabled = True
        else:
            msg += f"\n🩸 Máu Boss còn lại: **{self.hp:,} / {self.max_hp:,}** (Vòng {self.round_num}/10)"

        # Cập nhật tin nhắn boss để mô phỏng diễn biến tiếp theo
        embed = interaction.message.embeds[0] if interaction.message.embeds else None
        if embed:
            embed.set_field_at(0, name="🩸 Máu Boss", value=f"{self.hp:,} / {self.max_hp:,}", inline=True)
            embed.set_field_at(1, name="🔄 Tiến độ", value=f"Vòng {self.round_num} / 10", inline=True)

        await interaction.response.send_message(msg, ephemeral=False)
        try:
            await interaction.message.edit(embed=embed, view=self)
        except Exception:
            pass

    @discord.ui.button(
        label="BXH sát thương",
        style=discord.ButtonStyle.secondary, # Nút xám
        emoji="🏆",
        custom_id="rank_boss_demo"
    )
    async def rank_button(self, interaction: discord.Interaction, button: discord.ui.Button):
        await interaction.response.send_message("🏆 **BXH sát thương:**\n1. Top 1: 54,510\n2. Top 2: 28,160", ephemeral=True)

@bot.event
async def on_ready():
    print("=" * 60)
    print(f"🤖 Bot Demo đã sẵn sàng: {bot.user.name} (ID: {bot.user.id})")
    print("💡 Hướng dẫn test:")
    print("  1. Vào server/kênh Discord của bạn.")
    print("  2. Gõ lệnh: !boss")
    print("  3. Bot sẽ gửi tin nhắn có nút '⚔️ Đánh Boss'.")
    print("  4. Bật tool Auto Reactor và xem tool tự động bấm nút!")
    print("=" * 60)

@bot.command(name="boss")
async def spawn_boss(ctx):
    """Lệnh tạo boss giả lập: !boss"""
    embed = discord.Embed(
        title="⚔️ TRẬN ĐÁNH BOSS: Cattleya Glory",
        description="🔥 **Cattleya Glory** đạt 100 Nộ và tung **Phá Vỡ Giới Hạn — Cuồng Nộ Diệt Ngũ!**\n*Hãy bấm nút bên dưới để tấn công boss!*",
        color=0xe74c3c
    )
    embed.add_field(name="🩸 Máu Boss", value="291,665 / 428,430", inline=True)
    embed.add_field(name="🔄 Tiến độ", value="Vòng 1 / 10", inline=True)
    embed.add_field(name="⏳ Thời gian", value="Còn 06:00", inline=True)
    embed.set_thumbnail(url="https://cdn.discordapp.com/emojis/1042784747065380904.webp?size=96&quality=lossless")
    embed.set_footer(text="Bot Game Demo • Bấm Đánh Boss để gây sát thương")

    view = BossBattleView()
    await ctx.send(content="**[THÔNG BÁO BOSS XUẤT HIỆN]**", embed=embed, view=view)
    print(f"[BOT] Đã tạo trận Boss tại kênh #{ctx.channel.name} (ID: {ctx.channel.id})")

if __name__ == "__main__":
    if len(sys.argv) > 1:
        token = sys.argv[1].strip()
    else:
        print("\n=== BOT DISCORD DEMO TEST ĐÁNH BOSS ===")
        token = input("Nhập Bot Token của bạn (hoặc chạy: python demo_boss_bot.py <TOKEN>): ").strip()

    if not token:
        print("❌ Chưa nhập Token!")
        sys.exit(1)

    try:
        bot.run(token)
    except Exception as e:
        print(f"❌ Lỗi chạy bot: {e}")
