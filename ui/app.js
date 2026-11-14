// Khởi tạo cầu nối Tauri
const hasTauri = typeof window.__TAURI__ !== "undefined" && window.__TAURI__.core;
const invoke = hasTauri ? window.__TAURI__.core.invoke : async (cmd, args) => {
  console.log("[Mock Invoke]", cmd, args);
  return null;
};

// State toàn cục phía UI
let currentConfig = null;
let currentStatus = { is_running: false, session_reactions: 0, total_reactions: 0, total_channels_scanned: 0, active_account_name: "" };
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
    document.querySelectorAll(".screen").forEach(s => s.classList.remove("active"));
    
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
    document.getElementById("cfg-delay-max").value = currentConfig.bot_settings.delay_max ?? (currentConfig.bot_settings.delay || 2.5);
    document.getElementById("cfg-conc").value = currentConfig.bot_settings.concurrent_channels || 4;
    document.getElementById("cfg-skip-bots").checked = currentConfig.bot_settings.skip_bot_messages;
    document.getElementById("cfg-full-scan").checked = currentConfig.bot_settings.full_scan;

    // Gửi emoji tự động
    const es = currentConfig.emoji_sender || {};
    document.getElementById("cfg-es-enabled").checked = !!es.enabled;
    document.getElementById("cfg-es-channel").value = es.channel_id || "";
    document.getElementById("cfg-es-interval").value = es.interval_secs || 30;
    document.getElementById("cfg-es-animated").checked = !!es.allow_animated;

    // Filters
    document.getElementById("cfg-whitelist").value = (currentConfig.emoji_whitelist || []).join(", ");
    document.getElementById("cfg-blacklist").value = (currentConfig.emoji_blacklist || []).join(", ");

    renderAccounts();
    renderCategories();
  } catch (e) {
    console.error("Lỗi tải config:", e);
  }
}

// ---- Tiện ích UI ----
const esc = (v) => String(v ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));

const ICON_EDIT = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/></svg>';
const ICON_TRASH = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14M10 11v5M14 11v5"/></svg>';

const toastEl = document.getElementById("toast");
let toastTimer = null;
function toast(msg, type) {
  const text = String(msg ?? "");
  if (!type) type = /lỗi|không|vui lòng|error/i.test(text) ? "err" : "ok";
  toastEl.textContent = text;
  toastEl.className = "toast show " + type;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toastEl.classList.remove("show"), 2800);
}

// 3. Render danh sách tài khoản
function renderAccounts() {
  const container = document.getElementById("accounts-list");
  container.innerHTML = "";
  if (!currentConfig || !currentConfig.accounts || currentConfig.accounts.length === 0) {
    container.innerHTML = `<div class="empty">Chưa có tài khoản nào.<br>Bấm “Dán token” để thêm.</div>`;
    return;
  }

  const activeId = currentConfig.active_account_id;
  currentConfig.accounts.forEach((acc) => {
    const isActive = acc.id === activeId;
    const card = document.createElement("div");
    card.className = "row-card" + (isActive ? " active" : "");

    const masked = acc.token.length > 12 ? `${acc.token.slice(0, 6)}…${acc.token.slice(-4)}` : "••••••••";
    const initial = esc((acc.name || "?").trim().charAt(0).toUpperCase() || "?");

    card.innerHTML = `
      <div class="avatar">${initial}</div>
      <div class="row-info">
        <div class="row-title"><span class="t">${esc(acc.name)}</span>${isActive ? '<span class="chip">Đang dùng</span>' : ""}</div>
        <div class="row-sub">${esc(masked)}</div>
      </div>
      <div class="row-actions">
        ${!isActive ? `<button class="link-btn btn-select">Chọn</button>` : ""}
        <button class="icon-btn danger btn-del" aria-label="Xóa">${ICON_TRASH}</button>
      </div>
    `;

    card.querySelector(".btn-del").addEventListener("click", async () => {
      if (!confirm(`Xóa tài khoản “${acc.name}”?`)) return;
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
    container.innerHTML = `<div class="empty">Chưa có danh mục nào.<br>Bấm “Thêm” để bắt đầu.</div>`;
    return;
  }

  currentConfig.categories.forEach((cat, idx) => {
    const card = document.createElement("div");
    card.className = "row-card";

    const excCount = (cat.excluded_channels || []).length;
    card.innerHTML = `
      <div class="row-info">
        <div class="row-title"><span class="t">${esc(cat.name)}</span></div>
        <div class="row-sub">${esc(cat.id)} · ${excCount ? excCount + " kênh loại trừ" : "không loại trừ"}</div>
      </div>
      <div class="row-actions">
        <button class="icon-btn btn-edit-cat" aria-label="Sửa">${ICON_EDIT}</button>
        <button class="icon-btn danger btn-del-cat" aria-label="Xóa">${ICON_TRASH}</button>
      </div>
    `;

    card.querySelector(".btn-del-cat").addEventListener("click", async () => {
      if (!confirm(`Xóa danh mục “${cat.name}”?`)) return;
      currentConfig.categories.splice(idx, 1);
      await invoke("update_categories", { categories: currentConfig.categories });
      await loadConfig();
    });

    card.querySelector(".btn-edit-cat").addEventListener("click", () => {
      editingCatIndex = idx;
      document.getElementById("modal-cat-title").textContent = "Sửa danh mục";
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

    // Tắt Foreground Service khi bot đã dừng (kể cả tự dừng do lỗi token)
    if (!currentStatus.is_running && window.AndroidBridge) {
      try { window.AndroidBridge.stopService(); } catch (_) {}
    }

    dashAccName.textContent = currentStatus.active_account_name || "Chưa chọn";
    dashReactions.textContent = (currentStatus.session_reactions || 0).toLocaleString("vi-VN");
    document.getElementById("dash-total").textContent = (currentStatus.total_reactions || 0).toLocaleString("vi-VN");
    dashChannels.textContent = (currentStatus.total_channels_scanned || 0).toLocaleString("vi-VN");

    const running = !!currentStatus.is_running;
    badge.textContent = running ? "Đang chạy" : "Đã dừng";
    badge.className = "badge " + (running ? "badge-running" : "badge-stopped");
    btnToggle.textContent = running ? "Dừng hoạt động" : "Bắt đầu chạy";
    btnToggle.className = "btn btn-large btn-block " + (running ? "btn-stop" : "btn-start");
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
      // Android: giữ app sống khi thoát ra màn hình chính
      if (window.AndroidBridge) {
        try { window.AndroidBridge.startService(); } catch (_) {}
      }
    } catch (err) {
      appendLog("[LỖI] " + err);
      toast(err, "err");
    }
  }
  await updateStatus();
});

// 7. Thêm dòng log vào Terminal (tô màu theo mức độ, giới hạn số dòng để không phình RAM khi chạy nền)
const MAX_LOG_LINES = 400;
function appendLog(text) {
  const nearBottom = logTerminal.scrollHeight - logTerminal.scrollTop - logTerminal.clientHeight < 48;
  const m = /^\[(\d{2}:\d{2}:\d{2})\]\s*([\s\S]*)$/.exec(String(text));
  const ts = m ? m[1] : "";
  const body = m ? m[2] : String(text);

  const div = document.createElement("div");
  div.className = "log-line";
  if (/\[LỖI\]|\[!\]|lỗi/i.test(body)) div.classList.add("err");
  else if (/thành công|hoàn thành|bắt đầu hoạt động/i.test(body)) div.classList.add("ok");

  const tsEl = document.createElement("span");
  tsEl.className = "ts";
  tsEl.textContent = ts || "--:--:--";
  const msgEl = document.createElement("span");
  msgEl.textContent = body;
  div.append(tsEl, msgEl);
  logTerminal.appendChild(div);

  while (logTerminal.childElementCount > MAX_LOG_LINES) logTerminal.removeChild(logTerminal.firstChild);
  if (nearBottom) logTerminal.scrollTop = logTerminal.scrollHeight;
}

// Xóa log
document.getElementById("btn-clear-log").addEventListener("click", async () => {
  await invoke("clear_logs");
  logTerminal.innerHTML = '<div class="log-line muted"><span class="ts">--:--:--</span><span>Đã xóa nhật ký.</span></div>';
});

// 8. Lưu Cài đặt & Bộ lọc
document.getElementById("btn-save-settings").addEventListener("click", async () => {
  const dMin = parseFloat(document.getElementById("cfg-delay").value) || 1.0;
  const dMax = parseFloat(document.getElementById("cfg-delay-max").value) || dMin;
  const settings = {
    delay: Math.min(dMin, dMax),
    delay_max: Math.max(dMin, dMax),
    concurrent_channels: parseInt(document.getElementById("cfg-conc").value) || 4,
    skip_bot_messages: document.getElementById("cfg-skip-bots").checked,
    full_scan: document.getElementById("cfg-full-scan").checked,
    max_messages_per_channel: null
  };
  const channelId = document.getElementById("cfg-es-channel").value.trim();
  const enabled = document.getElementById("cfg-es-enabled").checked;
  if (channelId && !/^\d{15,25}$/.test(channelId)) {
    toast("ID kênh không hợp lệ (chỉ gồm chữ số).", "err");
    return;
  }
  if (enabled && !channelId) {
    toast("Vui lòng nhập ID kênh để bật gửi emoji.", "err");
    return;
  }
  const sender = {
    enabled,
    channel_id: channelId,
    interval_secs: Math.max(1, parseInt(document.getElementById("cfg-es-interval").value) || 30),
    allow_animated: document.getElementById("cfg-es-animated").checked
  };
  await invoke("update_bot_settings", { settings });
  await invoke("update_emoji_sender", { settings: sender });
  toast("Đã lưu cài đặt.");
});

document.getElementById("btn-reset-prog").addEventListener("click", async () => {
  if (confirm("Bạn có chắc chắn muốn xóa tiến trình đã quét để bot quét lại từ đầu?")) {
    await invoke("reset_progress");
    toast("Đã đặt lại tiến trình quét!");
  }
});

document.getElementById("btn-save-filters").addEventListener("click", async () => {
  const white = document.getElementById("cfg-whitelist").value.split(",").map(s => s.trim()).filter(Boolean);
  const black = document.getElementById("cfg-blacklist").value.split(",").map(s => s.trim()).filter(Boolean);
  await invoke("update_emoji_filters", { whitelist: white, blacklist: black });
  toast("Đã lưu bộ lọc Emoji thành công!");
});

// 9. Modals Account & Token Guide
const modalAcc = document.getElementById("modal-account");
const modalTokenGuide = document.getElementById("modal-token-guide");

// Mở/Đóng hướng dẫn lấy token 1-chạm
document.getElementById("btn-open-token-guide").addEventListener("click", () => {
  modalTokenGuide.classList.remove("hidden");
});
document.getElementById("btn-close-token-guide").addEventListener("click", () => {
  modalTokenGuide.classList.add("hidden");
});

// Sao chép đoạn mã JavaScript lấy token
const BOOKMARKLET_SCRIPT = `javascript:(function(){try{window.webpackChunkdiscord_app.push([[''],{},e=>{m=[];for(let c in e.c)m.push(e.c[c])}]);const t=m.find(m=>m?.exports?.default?.getToken!==void 0).exports.default.getToken();if(t){navigator.clipboard.writeText(t);alert("ĐÃ COPY TOKEN VÀO BỘ NHỚ TẠM!\\n\\nHãy mở lại App và bấm 'Dán token'.");}else{alert("Không tìm thấy Token. Hãy đảm bảo bạn đã đăng nhập Discord thành công trên trang web này.");}}catch(e){alert("Lỗi: "+e.message);}})();`;

document.getElementById("btn-copy-script").addEventListener("click", async () => {
  try {
    await navigator.clipboard.writeText(BOOKMARKLET_SCRIPT);
    const btn = document.getElementById("btn-copy-script");
    btn.textContent = "Đã sao chép";
    btn.className = "btn btn-success btn-block";
    setTimeout(() => {
      btn.textContent = "Sao chép mã lấy token";
      btn.className = "btn btn-primary btn-block";
    }, 2500);
  } catch (e) {
    toast("Không thể tự động sao chép: " + e);
  }
});

// Nút "Dán từ Clipboard" ngoài màn hình danh sách tài khoản
document.getElementById("btn-paste-clipboard").addEventListener("click", async () => {
  try {
    const text = await navigator.clipboard.readText();
    const token = text.trim();
    if (!token || token.length < 20) {
      toast("Bộ nhớ tạm không chứa Token Discord hợp lệ!\nHãy xem hướng dẫn lấy Token trước.");
      return;
    }
    const name = prompt("Nhập tên gợi nhớ cho tài khoản:", "Tài khoản Discord") || "Tài khoản Discord";
    await invoke("add_or_update_account", { name, token, accountId: null });
    toast("Đã thêm tài khoản thành công!");
    await loadConfig();
  } catch (e) {
    toast("Không thể đọc bộ nhớ tạm: " + e + "\nVui lòng dùng nút '+ Thêm thủ công' và dán vào ô.");
  }
});

// Nút dán bên trong Modal thêm tài khoản
document.getElementById("modal-acc-paste-btn").addEventListener("click", async () => {
  try {
    const text = await navigator.clipboard.readText();
    if (text) {
      document.getElementById("modal-acc-token").value = text.trim();
    }
  } catch (e) {
    toast("Vui lòng chạm giữ vào ô và chọn Dán (Paste).");
  }
});

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
    toast("Vui lòng nhập Discord Token!");
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
  document.getElementById("modal-cat-title").textContent = "Thêm danh mục";
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
    toast("Vui lòng nhập Category ID!");
    return;
  }
  if (!/^\d+$/.test(rawId)) {
    toast("Category ID chỉ gồm chữ số.", "err");
    return;
  }
  // Giữ ID dạng chuỗi: số snowflake lớn hơn 2^53 sẽ bị JS làm tròn sai
  const id = rawId;
  const excluded = document.getElementById("modal-cat-excluded").value
    .split(",")
    .map(s => s.trim())
    .filter(v => /^\d+$/.test(v));

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

// ---- Channel Picker (Chọn Category & Kênh trực quan từ Server) ----
const modalPicker = document.getElementById("modal-channel-picker");
const pickerServerSelect = document.getElementById("picker-server-select");
const pickerCategoriesList = document.getElementById("picker-categories-list");
const pickerStatus = document.getElementById("picker-status");

document.getElementById("btn-open-picker").addEventListener("click", async () => {
  modalPicker.classList.remove("hidden");
  await loadPickerServers();
});

document.getElementById("btn-close-channel-picker").addEventListener("click", () => {
  modalPicker.classList.add("hidden");
});

document.getElementById("btn-refresh-servers").addEventListener("click", async () => {
  await loadPickerServers();
});

async function loadPickerServers() {
  pickerServerSelect.innerHTML = '<option value="">-- Đang tải danh sách server... --</option>';
  pickerCategoriesList.innerHTML = "";
  pickerStatus.style.display = "block";
  pickerStatus.textContent = "Đang kết nối tới Discord để lấy danh sách server...";

  try {
    const guilds = await invoke("fetch_user_guilds");
    if (!guilds || guilds.length === 0) {
      pickerStatus.textContent = "Không tìm thấy server nào hoặc tài khoản chưa tham gia server nào.";
      pickerServerSelect.innerHTML = '<option value="">(Không có server)</option>';
      return;
    }

    pickerStatus.style.display = "none";
    pickerServerSelect.innerHTML = '<option value="">-- Chọn Server (' + guilds.length + ') --</option>' +
      guilds.map(g => `<option value="${esc(g.id)}">${esc(g.name)}</option>`).join("");
  } catch (err) {
    pickerStatus.style.display = "block";
    pickerStatus.textContent = "Lỗi tải server: " + err;
    pickerServerSelect.innerHTML = '<option value="">(Lỗi tải)</option>';
    toast(err, "err");
  }
}

pickerServerSelect.addEventListener("change", async () => {
  const guildId = pickerServerSelect.value;
  pickerCategoriesList.innerHTML = "";
  if (!guildId) return;

  pickerStatus.style.display = "block";
  pickerStatus.textContent = "Đang tải danh mục và các kênh của server...";

  try {
    const channels = await invoke("fetch_guild_channels", { guildId });
    pickerStatus.style.display = "none";

    if (!channels || channels.length === 0) {
      pickerCategoriesList.innerHTML = '<div class="empty">Server này không có kênh text nào.</div>';
      return;
    }

    // Tách categories (channel_type === 4) và text channels (channel_type === 0)
    const categories = channels.filter(c => c.channel_type === 4);
    const textChannels = channels.filter(c => c.channel_type === 0);

    if (categories.length === 0) {
      pickerCategoriesList.innerHTML = '<div class="empty">Server này không chia danh mục (Category).</div>';
      return;
    }

    categories.forEach(cat => {
      const children = textChannels.filter(ch => ch.parent_id === cat.id);
      const catCard = document.createElement("div");
      catCard.className = "row-card";
      catCard.style.flexDirection = "column";
      catCard.style.alignItems = "stretch";
      catCard.style.gap = "8px";

      // Kiểm tra xem category này đã có trong cấu hình chưa
      const isConfigured = (currentConfig?.categories || []).some(c => String(c.id) === String(cat.id));

      let channelsHtml = children.map(ch => `
        <label style="display:flex; align-items:center; gap:8px; font-size:12px; color:var(--text); padding:3px 0; cursor:pointer;">
          <input type="checkbox" class="ch-check" data-id="${esc(ch.id)}" checked style="accent-color:var(--accent);">
          <span># ${esc(ch.name || ch.id)}</span>
        </label>
      `).join("");

      if (!channelsHtml) {
        channelsHtml = '<div class="small text-muted" style="font-style:italic;">(Không có kênh text nào trong category này)</div>';
      }

      catCard.innerHTML = `
        <div style="display:flex; justify-content:space-between; align-items:center; border-bottom:1px solid var(--border); padding-bottom:6px;">
          <div>
            <strong style="font-size:14px;">${esc(cat.name || "Danh mục")}</strong>
            <div class="small text-muted">ID: ${esc(cat.id)} · ${children.length} kênh text</div>
          </div>
          <button class="btn btn-sm btn-primary btn-add-this-cat">
            ${isConfigured ? "Cập nhật" : "+ Thêm"}
          </button>
        </div>
        <div class="channels-box" style="display:flex; flex-direction:column; padding-left:4px;">
          ${channelsHtml}
        </div>
      `;

      // Nút thêm/cập nhật category
      catCard.querySelector(".btn-add-this-cat").addEventListener("click", async () => {
        // Thu thập các kênh không được tích (unchecked) làm excluded_channels
        const excluded = [];
        catCard.querySelectorAll(".ch-check").forEach(chk => {
          if (!chk.checked) {
            excluded.push(chk.getAttribute("data-id"));
          }
        });

        const newCat = {
          id: cat.id,
          name: cat.name || ("Category " + cat.id),
          excluded_channels: excluded
        };

        if (!currentConfig.categories) currentConfig.categories = [];
        const existingIdx = currentConfig.categories.findIndex(c => String(c.id) === String(cat.id));
        if (existingIdx >= 0) {
          currentConfig.categories[existingIdx] = newCat;
        } else {
          currentConfig.categories.push(newCat);
        }

        await invoke("update_categories", { categories: currentConfig.categories });
        toast(`Đã thêm “${cat.name}” (${children.length - excluded.length}/${children.length} kênh)!`, "ok");
        renderCategories();
      });

      pickerCategoriesList.appendChild(catCard);
    });
  } catch (err) {
    pickerStatus.style.display = "block";
    pickerStatus.textContent = "Lỗi tải kênh: " + err;
    toast(err, "err");
  }
});

// Chạm nền mờ để đóng modal
document.querySelectorAll(".modal-overlay").forEach((ov) => {
  ov.addEventListener("click", (e) => { if (e.target === ov) ov.classList.add("hidden"); });
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
