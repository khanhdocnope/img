// Khởi tạo cầu nối: Tự động nhận diện Tauri (Desktop/Android) hoặc Web Server (Render/Cloud)
const hasTauri = typeof window.__TAURI__ !== "undefined" && window.__TAURI__.core;
const invoke = hasTauri
  ? window.__TAURI__.core.invoke
  : async (cmd, args = {}) => {
      const res = await fetch(`/api/invoke/${cmd}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(args || {}),
      });
      if (!res.ok) {
        const errText = await res.text().catch(() => "");
        throw new Error(errText || `Lỗi máy chủ (${res.status})`);
      }
      return await res.json().catch(() => null);
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
    if (targetId === "tab-accounts") checkAllAccounts(2 * 60 * 1000);
  });
});

// 2. Tải cấu hình từ Rust backend
async function loadConfig() {
  try {
    currentConfig = await invoke("get_config");
    if (!currentConfig) return;

    // Settings
    document.getElementById("cfg-delay").value = currentConfig.bot_settings.delay !== undefined ? currentConfig.bot_settings.delay : 0.3;
    document.getElementById("cfg-delay-max").value = currentConfig.bot_settings.delay_max !== undefined ? currentConfig.bot_settings.delay_max : 0.6;
    document.getElementById("cfg-conc").value = currentConfig.bot_settings.concurrent_channels || 4;
    document.getElementById("cfg-skip-bots").checked = currentConfig.bot_settings.skip_bot_messages;
    document.getElementById("cfg-full-scan").checked = currentConfig.bot_settings.full_scan;
    document.getElementById("cfg-max-msgs").value = currentConfig.bot_settings.max_messages_per_channel || "";

    // Gửi emoji tự động
    const es = currentConfig.emoji_sender || {};
    document.getElementById("cfg-es-enabled").checked = !!es.enabled;
    document.getElementById("cfg-es-channel").value = es.channel_id || "";
    document.getElementById("cfg-es-interval").value = es.interval_secs || 30;
    document.getElementById("cfg-es-animated").checked = !!es.allow_animated;


    renderAccounts();
    renderCategories();
    checkAllAccounts(); // chạy nền, không chặn giao diện
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

// ---- Kiểm tra token ----
const tokenStatus = {};            // accountId -> { state, msg, user, at }
const TOKEN_STALE_MS = 5 * 60 * 1000;
let checkingTokens = false;

function cleanToken(raw) {
  // Bỏ khoảng trắng và dấu nháy thừa khi dán token từ localStorage / console
  return String(raw || "").trim().replace(/^["'`]+|["'`]+$/g, "").trim();
}

async function runTokenCheck(token) {
  try {
    const r = await invoke("check_token", { token });
    if (r && r.status) return r;
  } catch (_) {}
  return { status: "unknown", username: null, message: "Không kiểm tra được lúc này." };
}

async function checkAccount(acc) {
  tokenStatus[acc.id] = { ...(tokenStatus[acc.id] || {}), state: "checking" };
  renderAccounts();
  const r = await runTokenCheck(acc.token);
  tokenStatus[acc.id] = { state: r.status, msg: r.message, user: r.username, at: Date.now() };
  renderAccounts();
}

// Kiểm tra lần lượt các tài khoản chưa kiểm tra hoặc đã cũ (không chặn giao diện)
async function checkAllAccounts(maxAgeMs = TOKEN_STALE_MS) {
  if (checkingTokens || !currentConfig || !currentConfig.accounts) return;
  checkingTokens = true;
  try {
    for (const acc of [...currentConfig.accounts]) {
      const st = tokenStatus[acc.id];
      if (st && st.at && Date.now() - st.at < maxAgeMs) continue;
      await checkAccount(acc);
      await new Promise((res) => setTimeout(res, 400)); // tránh dồn request
    }
  } finally {
    checkingTokens = false;
  }
}

// Kiểm tra token trước khi lưu. Trả về { ok, name, result }
async function verifyBeforeSave(token, typedName) {
  const r = await runTokenCheck(token);
  if (r.status === "invalid") {
    toast("Token không dùng được:\n" + r.message, "err");
    return { ok: false };
  }
  if (r.status === "unknown" && !confirm(r.message + "\nVẫn lưu token này?")) {
    return { ok: false };
  }
  const name = typedName || r.username || "Tài khoản Discord";
  return { ok: true, name, result: r };
}

async function saveAccount(token, typedName) {
  const v = await verifyBeforeSave(token, typedName);
  if (!v.ok) return false;
  const id = await invoke("add_or_update_account", { name: v.name, token, accountId: null });
  if (id) tokenStatus[id] = { state: v.result.status, msg: v.result.message, user: v.result.username, at: Date.now() };
  toast(v.result.status === "valid" ? `Đã thêm “${v.name}”. Token còn hoạt động.` : "Đã thêm tài khoản.", "ok");
  await loadConfig();
  return true;
}

const STATUS_TEXT = { valid: "Còn hoạt động", invalid: "Token hỏng", checking: "Đang kiểm tra…", unknown: "Chưa xác định" };

// Tài khoản chính: tài khoản đang chọn nếu được bật, nếu không thì tài khoản bật đầu tiên
function primaryAccountId() {
  const accs = (currentConfig && currentConfig.accounts) || [];
  const enabled = accs.filter((a) => a.enabled);
  if (enabled.some((a) => a.id === currentConfig.active_account_id)) return currentConfig.active_account_id;
  return enabled.length ? enabled[0].id : null;
}

// 3. Render danh sách tài khoản
function renderAccounts() {
  const container = document.getElementById("accounts-list");
  container.innerHTML = "";
  if (!currentConfig || !currentConfig.accounts || currentConfig.accounts.length === 0) {
    container.innerHTML = `<div class="empty">Chưa có tài khoản nào.<br>Bấm “Dán token” để thêm.</div>`;
    return;
  }

  const primaryId = primaryAccountId();
  const enabledCount = currentConfig.accounts.filter((a) => a.enabled).length;

  currentConfig.accounts.forEach((acc) => {
    const isPrimary = acc.id === primaryId;
    const st = tokenStatus[acc.id] || { state: "unknown", msg: "Chạm để kiểm tra" };
    const card = document.createElement("div");
    card.className = "row-card" + (acc.enabled ? " active" : "") + (st.state === "invalid" ? " bad" : "");

    const masked = acc.token.length > 12 ? `${acc.token.slice(0, 6)}…${acc.token.slice(-4)}` : "••••••••";
    const initial = esc((acc.name || "?").trim().charAt(0).toUpperCase() || "?");
    const label = STATUS_TEXT[st.state] || STATUS_TEXT.unknown;
    const extra = st.state === "valid" && st.user ? ` · ${st.user}` : "";
    const showPrimaryChip = acc.enabled && enabledCount > 1 && isPrimary;
    const canMakePrimary = acc.enabled && enabledCount > 1 && !isPrimary;

    card.innerHTML = `
      <div class="avatar">${initial}</div>
      <div class="row-info">
        <div class="row-title"><span class="t">${esc(acc.name)}</span>${showPrimaryChip ? '<span class="chip">Chính</span>' : ""}</div>
        <div class="row-sub">${esc(masked)}</div>
        <span class="status st-${esc(st.state)}" title="${esc(st.msg || "")}">${esc(label + extra)}</span>
      </div>
      <div class="row-actions">
        ${canMakePrimary ? `<button class="link-btn btn-make-primary">Đặt chính</button>` : ""}
        <label class="switch" title="Chạy tài khoản này">
          <input type="checkbox" class="acc-run" ${acc.enabled ? "checked" : ""}>
          <span class="slider"></span>
        </label>
        <button class="icon-btn danger btn-del" aria-label="Xóa">${ICON_TRASH}</button>
      </div>
    `;

    card.querySelector(".status").addEventListener("click", async () => {
      await checkAccount(acc);
      const s2 = tokenStatus[acc.id];
      if (s2 && s2.msg) toast(s2.msg, s2.state === "valid" ? "ok" : s2.state === "invalid" ? "err" : undefined);
    });

    card.querySelector(".acc-run").addEventListener("change", async (e) => {
      const want = e.target.checked;
      if (want && tokenStatus[acc.id] && tokenStatus[acc.id].state === "invalid") {
        toast("Token của tài khoản này đã hỏng, không thể bật.", "err");
        e.target.checked = false;
        return;
      }
      try {
        await invoke("set_account_enabled", { accountId: acc.id, enabled: want });
        if (currentStatus && currentStatus.is_running) toast("Áp dụng ở lần bấm Bắt đầu tiếp theo.", "ok");
      } catch (err) {
        toast(err, "err");
      }
      await loadConfig();
    });

    card.querySelector(".btn-del").addEventListener("click", async () => {
      if (!confirm(`Xóa tài khoản “${acc.name}”?`)) return;
      await invoke("delete_account", { accountId: acc.id });
      delete tokenStatus[acc.id];
      await loadConfig();
    });

    const mk = card.querySelector(".btn-make-primary");
    if (mk) {
      mk.addEventListener("click", async () => {
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

const WORKER_TEXT = { starting: "Sắp bắt đầu", running: "Đang chạy", cooldown: "Tạm nghỉ", error: "Lỗi", stopped: "Đã dừng" };

// Hiển thị trạng thái từng tài khoản đang chạy (chỉ khi có từ 2 tài khoản trở lên)
function renderWorkers(workers) {
  const box = document.getElementById("dash-workers");
  if (!box) return;
  if (workers.length < 2) {
    box.innerHTML = "";
    box.classList.add("hidden");
    return;
  }
  box.classList.remove("hidden");
  box.innerHTML = workers
    .map(
      (w) => `<div class="worker w-${esc(w.state)}" title="${esc(w.note || "")}">
        <span class="w-name">${esc(w.name)}</span>
        <span class="w-count">${Number(w.reactions || 0).toLocaleString("vi-VN")}</span>
        <span class="w-state">${esc(WORKER_TEXT[w.state] || w.state)}${w.note && w.state !== "running" ? " · " + esc(w.note) : ""}</span>
      </div>`
    )
    .join("");
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

    renderWorkers(currentStatus.workers || []);

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
    const badAcc = ((currentConfig && currentConfig.accounts) || []).find(
      (a) => a.enabled && tokenStatus[a.id] && tokenStatus[a.id].state === "invalid"
    );
    if (badAcc) {
      toast(`Token của “${badAcc.name}” đã hỏng.\nHãy tắt hoặc thay token ở tab Tài khoản.`, "err");
      return;
    }
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
  let delayMin = Math.max(0.05, parseFloat(document.getElementById("cfg-delay").value) || 0.3);
  let delayMax = Math.max(0.05, parseFloat(document.getElementById("cfg-delay-max").value) || delayMin);
  if (delayMax < delayMin) [delayMin, delayMax] = [delayMax, delayMin];
  const maxMsgsVal = document.getElementById("cfg-max-msgs").value.trim();
  const maxMessages = maxMsgsVal ? parseInt(maxMsgsVal) : null;
  const settings = {
    delay: delayMin,
    delay_max: delayMax,
    concurrent_channels: parseInt(document.getElementById("cfg-conc").value) || 4,
    skip_bot_messages: document.getElementById("cfg-skip-bots").checked,
    full_scan: document.getElementById("cfg-full-scan").checked,
    scan_history: false,
    max_messages_per_channel: maxMessages
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
  let token = "";
  try {
    token = cleanToken(await navigator.clipboard.readText());
  } catch (e) {
    toast("Không thể đọc bộ nhớ tạm.\nHãy dùng nút “Thêm tay” và dán vào ô.");
    return;
  }
  if (!token || token.length < 20) {
    toast("Bộ nhớ tạm không chứa token Discord hợp lệ.\nHãy xem hướng dẫn lấy token trước.");
    return;
  }
  toast("Đang kiểm tra token…", "ok");
  try {
    await saveAccount(token, ""); // tên tự lấy theo tài khoản Discord
  } catch (e) {
    toast("Không thể thêm tài khoản: " + e);
  }
});

// Nút dán bên trong Modal thêm tài khoản
document.getElementById("modal-acc-paste-btn").addEventListener("click", async () => {
  try {
    const text = await navigator.clipboard.readText();
    if (text) {
      document.getElementById("modal-acc-token").value = cleanToken(text);
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
  const typedName = document.getElementById("modal-acc-name").value.trim();
  const token = cleanToken(document.getElementById("modal-acc-token").value);
  if (!token) {
    toast("Vui lòng nhập Discord Token!");
    return;
  }
  const btn = document.getElementById("modal-acc-save");
  btn.disabled = true;
  btn.textContent = "Đang kiểm tra…";
  try {
    if (await saveAccount(token, typedName)) modalAcc.classList.add("hidden");
  } catch (e) {
    toast("Không thể thêm tài khoản: " + e);
  } finally {
    btn.disabled = false;
    btn.textContent = "Lưu";
  }
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

// Chạm nền mờ để đóng modal
document.querySelectorAll(".modal-overlay").forEach((ov) => {
  ov.addEventListener("click", (e) => { if (e.target === ov) ov.classList.add("hidden"); });
});

// 11. Đăng ký lắng nghe sự kiện Log từ Rust
if (hasTauri && window.__TAURI__.event) {
  window.__TAURI__.event.listen("log", (event) => {
    appendLog(event.payload);
  });
} else {
  // Trình duyệt Web (Render/Cloud) -> Dùng Server-Sent Events (SSE) để nhận log trực tiếp
  try {
    const sse = new EventSource("/api/logs/stream");
    sse.onmessage = (event) => {
      if (event.data) {
        try {
          const parsed = JSON.parse(event.data);
          appendLog(parsed);
        } catch (_) {
          appendLog(event.data);
        }
      }
    };
  } catch (err) {
    console.warn("SSE error:", err);
  }
}

// 12. Sao chép cấu hình JSON cho Render / Cloud
const btnExportRender = document.getElementById("btn-export-render-config");
if (btnExportRender) {
  btnExportRender.addEventListener("click", async () => {
    try {
      const cfg = await invoke("get_config");
      const jsonStr = JSON.stringify(cfg, null, 2);
      await navigator.clipboard.writeText(jsonStr);
      toast("Đã copy cấu hình JSON! Dán vào biến BOT_CONFIG trên Render.", "success");
    } catch (e) {
      toast("Không thể copy cấu hình: " + e, "err");
    }
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
