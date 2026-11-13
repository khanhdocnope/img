// Khởi tạo cầu nối Tauri
const hasTauri = typeof window.__TAURI__ !== "undefined" && window.__TAURI__.core;
const invoke = hasTauri ? window.__TAURI__.core.invoke : async (cmd, args) => {
  console.log("[Mock Invoke]", cmd, args);
  return null;
};

// State toàn cục phía UI
let currentConfig = null;
let currentStatus = { is_running: false, session_reactions: 0, total_channels_scanned: 0, active_account_name: "" };
let editingCatIndex = null;

// DOM Elements
const badge = document.getElementById("status-badge");
const btnToggle = document.getElementById("btn-toggle-bot");
const dashAccName = document.getElementById("dash-account-name");
const dashReactions = document.getElementById("dash-reactions");
const dashChannels = document.getElementById("dash-channels");
const logTerminal = document.getElementById("log-terminal");

// 1. Chuyển đổi Tabs
document.querySelectorAll(".nav-item").forEach(btn => {
  btn.addEventListener("click", () => {
    document.querySelectorAll(".nav-item").forEach(b => b.classList.remove("active"));
    document.querySelectorAll(".tab-screen").forEach(s => s.classList.remove("active"));
    
    btn.classList.add("active");
    const targetId = btn.getAttribute("data-tab");
    const screen = document.getElementById(targetId);
    if (screen) screen.classList.add("active");
  });
});

// 2. Tải cấu hình từ Rust backend
async function loadConfig() {
  try {
    currentConfig = await invoke("get_config");
    if (!currentConfig) return;

    // Settings
    document.getElementById("cfg-delay").value = currentConfig.bot_settings.delay || 1.0;
    document.getElementById("cfg-conc").value = currentConfig.bot_settings.concurrent_channels || 4;
    document.getElementById("cfg-skip-bots").checked = currentConfig.bot_settings.skip_bot_messages;
    document.getElementById("cfg-full-scan").checked = currentConfig.bot_settings.full_scan;

    // Filters
    document.getElementById("cfg-whitelist").value = (currentConfig.emoji_whitelist || []).join(", ");
    document.getElementById("cfg-blacklist").value = (currentConfig.emoji_blacklist || []).join(", ");

    renderAccounts();
    renderCategories();
  } catch (e) {
    console.error("Lỗi tải config:", e);
  }
}

// 3. Render danh sách tài khoản
function renderAccounts() {
  const container = document.getElementById("accounts-list");
  container.innerHTML = "";
  if (!currentConfig || !currentConfig.accounts || currentConfig.accounts.length === 0) {
    container.innerHTML = `<div class="card text-muted">Chưa có tài khoản nào. Bấm "+ Thêm tài khoản" ở trên.</div>`;
    return;
  }

  const activeId = currentConfig.active_account_id;
  currentConfig.accounts.forEach(acc => {
    const isActive = acc.id === activeId;
    const card = document.createElement("div");
    card.className = "item-card";
    
    const masked = acc.token.length > 12 ? `${acc.token.slice(0, 6)}...${acc.token.slice(-4)}` : "********";
    
    card.innerHTML = `
      <div class="item-info">
        <div class="item-title">${isActive ? '<span style="color:var(--accent-green)">[KÍCH HOẠT] </span>' : ''}${acc.name}</div>
        <div class="item-subtitle">${masked}</div>
      </div>
      <div class="item-actions">
        ${!isActive ? `<button class="btn btn-sm btn-primary btn-select" data-id="${acc.id}">Chọn</button>` : ''}
        <button class="btn btn-sm btn-danger btn-del" data-id="${acc.id}">Xóa</button>
      </div>
    `;

    card.querySelector(".btn-del").addEventListener("click", async () => {
      await invoke("delete_account", { accountId: acc.id });
      await loadConfig();
    });

    const selectBtn = card.querySelector(".btn-select");
    if (selectBtn) {
      selectBtn.addEventListener("click", async () => {
        await invoke("set_active_account", { accountId: acc.id });
        await loadConfig();
      });
    }

    container.appendChild(card);
  });
}

// 4. Render danh mục
function renderCategories() {
  const container = document.getElementById("categories-list");
  container.innerHTML = "";
  if (!currentConfig || !currentConfig.categories || currentConfig.categories.length === 0) {
    container.innerHTML = `<div class="card text-muted">Chưa có Category nào. Bấm "+ Thêm Category".</div>`;
    return;
  }

  currentConfig.categories.forEach((cat, idx) => {
    const card = document.createElement("div");
    card.className = "card";
    card.style.display = "flex";
    card.style.flexDirection = "column";
    card.style.gap = "8px";

    const excCount = (cat.excluded_channels || []).length;
    card.innerHTML = `
      <div style="display:flex; justify-content:space-between; align-items:center;">
        <strong>${cat.name} (ID: ${cat.id})</strong>
        <div style="display:flex; gap:6px;">
          <button class="btn btn-sm btn-primary btn-edit-cat" data-idx="${idx}">Sửa</button>
          <button class="btn btn-sm btn-danger btn-del-cat" data-idx="${idx}">Xóa</button>
        </div>
      </div>
      <div class="text-muted small">Kênh loại trừ (${excCount}): ${(cat.excluded_channels || []).join(", ") || "(Không)"}</div>
    `;

    card.querySelector(".btn-del-cat").addEventListener("click", async () => {
      currentConfig.categories.splice(idx, 1);
      await invoke("update_categories", { categories: currentConfig.categories });
      await loadConfig();
    });

    card.querySelector(".btn-edit-cat").addEventListener("click", () => {
      editingCatIndex = idx;
      document.getElementById("modal-cat-title").textContent = "Sửa Category";
      document.getElementById("modal-cat-name").value = cat.name;
      document.getElementById("modal-cat-id").value = cat.id;
      document.getElementById("modal-cat-excluded").value = (cat.excluded_channels || []).join(", ");
      document.getElementById("modal-category").classList.remove("hidden");
    });

    container.appendChild(card);
  });
}

// 5. Cập nhật trạng thái Bot (Polling)
async function updateStatus() {
  try {
    currentStatus = await invoke("get_status");
    if (!currentStatus) return;

    dashAccName.textContent = currentStatus.active_account_name || "Chưa chọn";
    dashReactions.textContent = currentStatus.session_reactions || 0;
    dashChannels.textContent = currentStatus.total_channels_scanned || 0;

    if (currentStatus.is_running) {
      badge.textContent = "● ĐANG CHẠY";
      badge.className = "badge badge-running";
      btnToggle.textContent = "DỪNG HOẠT ĐỘNG";
      btnToggle.className = "btn btn-danger btn-large btn-block";
    } else {
      badge.textContent = "● ĐÃ DỪNG";
      badge.className = "badge badge-stopped";
      btnToggle.textContent = "BẮT ĐẦU CHẠY BOT";
      btnToggle.className = "btn btn-success btn-large btn-block";
    }
  } catch (e) {
    console.error("Lỗi cập nhật trạng thái:", e);
  }
}

// 6. Xử lý Start / Stop
btnToggle.addEventListener("click", async () => {
  if (currentStatus.is_running) {
    await invoke("stop_bot");
    appendLog("Đã yêu cầu dừng bot.");
  } else {
    try {
      appendLog("Đang khởi động bot...");
      await invoke("start_bot");
    } catch (err) {
      appendLog("[LỖI] " + err);
      alert(err);
    }
  }
  await updateStatus();
});

// 7. Thêm dòng log vào Terminal
function appendLog(text) {
  const div = document.createElement("div");
  div.className = "log-line";
  div.textContent = text;
  logTerminal.appendChild(div);
  logTerminal.scrollTop = logTerminal.scrollHeight;
}

// Xóa log
document.getElementById("btn-clear-log").addEventListener("click", async () => {
  await invoke("clear_logs");
  logTerminal.innerHTML = '<div class="log-line text-muted">[00:00:00] Đã xóa nhật ký.</div>';
});

// 8. Lưu Cài đặt & Bộ lọc
document.getElementById("btn-save-settings").addEventListener("click", async () => {
  const settings = {
    delay: parseFloat(document.getElementById("cfg-delay").value) || 1.0,
    concurrent_channels: parseInt(document.getElementById("cfg-conc").value) || 4,
    skip_bot_messages: document.getElementById("cfg-skip-bots").checked,
    full_scan: document.getElementById("cfg-full-scan").checked,
    max_messages_per_channel: null
  };
  await invoke("update_bot_settings", { settings });
  alert("Đã lưu cài đặt tham số thành công!");
});

document.getElementById("btn-reset-prog").addEventListener("click", async () => {
  if (confirm("Bạn có chắc chắn muốn xóa tiến trình đã quét để bot quét lại từ đầu?")) {
    await invoke("reset_progress");
    alert("Đã đặt lại tiến trình quét!");
  }
});

document.getElementById("btn-save-filters").addEventListener("click", async () => {
  const white = document.getElementById("cfg-whitelist").value.split(",").map(s => s.trim()).filter(Boolean);
  const black = document.getElementById("cfg-blacklist").value.split(",").map(s => s.trim()).filter(Boolean);
  await invoke("update_emoji_filters", { whitelist: white, blacklist: black });
  alert("Đã lưu bộ lọc Emoji thành công!");
});

// 9. Modals Account
const modalAcc = document.getElementById("modal-account");
document.getElementById("btn-open-add-acc").addEventListener("click", () => {
  document.getElementById("modal-acc-name").value = "";
  document.getElementById("modal-acc-token").value = "";
  modalAcc.classList.remove("hidden");
});
document.getElementById("modal-acc-cancel").addEventListener("click", () => modalAcc.classList.add("hidden"));
document.getElementById("modal-acc-save").addEventListener("click", async () => {
  const name = document.getElementById("modal-acc-name").value.trim() || "Tài khoản Discord";
  const token = document.getElementById("modal-acc-token").value.trim();
  if (!token) {
    alert("Vui lòng nhập Discord Token!");
    return;
  }
  await invoke("add_or_update_account", { name, token, accountId: null });
  modalAcc.classList.add("hidden");
  await loadConfig();
});

// 10. Modals Category
const modalCat = document.getElementById("modal-category");
document.getElementById("btn-open-add-cat").addEventListener("click", () => {
  editingCatIndex = null;
  document.getElementById("modal-cat-title").textContent = "Thêm Category";
  document.getElementById("modal-cat-name").value = "";
  document.getElementById("modal-cat-id").value = "";
  document.getElementById("modal-cat-excluded").value = "";
  modalCat.classList.remove("hidden");
});
document.getElementById("modal-cat-cancel").addEventListener("click", () => modalCat.classList.add("hidden"));
document.getElementById("modal-cat-save").addEventListener("click", async () => {
  const name = document.getElementById("modal-cat-name").value.trim() || "Category";
  const rawId = document.getElementById("modal-cat-id").value.trim();
  if (!rawId) {
    alert("Vui lòng nhập Category ID!");
    return;
  }
  const id = parseInt(rawId);
  const excluded = document.getElementById("modal-cat-excluded").value
    .split(",")
    .map(s => parseInt(s.trim()))
    .filter(n => !isNaN(n));

  const catObj = { id, name, excluded_channels: excluded };

  if (editingCatIndex !== null && editingCatIndex >= 0) {
    currentConfig.categories[editingCatIndex] = catObj;
  } else {
    currentConfig.categories.push(catObj);
  }

  await invoke("update_categories", { categories: currentConfig.categories });
  modalCat.classList.add("hidden");
  await loadConfig();
});

// 11. Đăng ký lắng nghe sự kiện Log từ Rust
if (hasTauri && window.__TAURI__.event) {
  window.__TAURI__.event.listen("log", (event) => {
    appendLog(event.payload);
  });
}

// Khởi chạy khi load trang
(async () => {
  await loadConfig();
  await updateStatus();
  
  // Tải log cũ
  try {
    const logs = await invoke("get_logs");
    if (logs && logs.length > 0) {
      logTerminal.innerHTML = "";
      logs.forEach(l => appendLog(l));
    }
  } catch (e) {}

  // Lặp cập nhật trạng thái định kỳ 1.5s
  setInterval(updateStatus, 1500);
})();
