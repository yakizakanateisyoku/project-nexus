// ========================================
// Project Nexus — Chat UI Controller
// Phase 3: Context Monitoring + Token Tracking + Remote Management
// ========================================

const { invoke } = window.__TAURI__.core;

// DOM Elements
let messagesEl;
let chatInputEl;
let chatFormEl;
let sendBtnEl;
let contextBadgeEl;
let machineListEl;
let remotePanelEl;
let remoteTargetLabel;
let remoteCmdInput;
let remoteExecBtn;
let remoteOutputEl;

// State
let isProcessing = false;
let messageHistory = [];
let currentTokenStats = null;
let currentModel = "claude-sonnet-4-5-20250929";
let selectedRemoteMachine = null;
let machineStatuses = [];
let statusPollTimer = null;
const STATUS_POLL_INTERVAL = 15000; // 15秒間隔（軽量化）

// Model pricing (per million tokens)
const MODEL_PRICING = {
  "claude-sonnet-4-5-20250929": { input: 3.0, output: 15.0, contextWindow: 200000 },
  "claude-haiku-4-5-20251001": { input: 0.80, output: 4.0, contextWindow: 200000 },
};

// Context warning thresholds
const CONTEXT_WARN_PERCENT = 75;
const CONTEXT_CRITICAL_PERCENT = 90;

// ========================================
// Initialization
// ========================================
window.addEventListener("DOMContentLoaded", () => {
  messagesEl = document.getElementById("messages");
  chatInputEl = document.getElementById("chat-input");
  chatFormEl = document.getElementById("chat-form");
  sendBtnEl = document.getElementById("send-btn");
  contextBadgeEl = document.getElementById("context-badge");
  machineListEl = document.getElementById("machine-list");
  remotePanelEl = document.getElementById("remote-panel");
  remoteTargetLabel = document.getElementById("remote-target-label");
  remoteCmdInput = document.getElementById("remote-cmd-input");
  remoteExecBtn = document.getElementById("remote-exec-btn");
  remoteOutputEl = document.getElementById("remote-output");

  // Form submit
  chatFormEl.addEventListener("submit", (e) => {
    e.preventDefault();
    handleSend();
  });

  // Textarea Enter handling
  chatInputEl.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  });

  chatInputEl.addEventListener("input", () => {
    autoResizeTextarea();
  });

  // Model selector
  const modelSelect = document.getElementById("model-select");
  if (modelSelect) {
    invoke("get_current_model").then((model) => {
      modelSelect.value = model;
      currentModel = model;
    });

    modelSelect.addEventListener("change", async (e) => {
      try {
        const result = await invoke("set_model", { modelId: e.target.value });
        currentModel = e.target.value;
        addMessage("system", result);
      } catch (err) {
        addMessage("system", `Error: ${err}`);
        const current = await invoke("get_current_model");
        modelSelect.value = current;
        currentModel = current;
      }
    });
  }

  // New chat button
  const newChatBtn = document.getElementById("new-chat-btn");
  if (newChatBtn) {
    newChatBtn.addEventListener("click", async () => {
      try {
        await invoke("clear_history");
        messagesEl.innerHTML = "";
        messageHistory = [];
        currentTokenStats = null;
        updateContextBadge(null);
        removeContextWarning();
        addMessage("system", "会話履歴をクリアしました");
      } catch (err) {
        addMessage("system", `Error: ${err}`);
      }
    });
  }

  // Remote command panel
  remoteExecBtn.addEventListener("click", handleRemoteExec);
  remoteCmdInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      handleRemoteExec();
    }
  });

  // Initial machine status + start polling
  refreshMachineStatus();
  statusPollTimer = setInterval(refreshMachineStatus, STATUS_POLL_INTERVAL);

  chatInputEl.focus();
});

// ========================================
// Message Handling
// ========================================
async function handleSend() {
  const text = chatInputEl.value.trim();
  if (!text || isProcessing) return;

  addMessage("user", text);
  chatInputEl.value = "";
  autoResizeTextarea();
  setProcessing(true);

  try {
    const response = await invoke("send_message", { message: text });
    // Phase 3: response is now { text, token_stats }
    addMessage("assistant", response.text);
    currentTokenStats = response.token_stats;
    updateContextBadge(response.token_stats);
    checkContextWarning(response.token_stats);
  } catch (err) {
    addMessage("system", `Error: ${err}`);
  } finally {
    setProcessing(false);
    chatInputEl.focus();
  }
}

function addMessage(role, content) {
  // Remove typing indicator if present
  const typingEl = messagesEl.querySelector(".typing-message");
  if (typingEl) typingEl.remove();

  const msgEl = document.createElement("div");
  msgEl.className = `message ${role}`;

  let inner = "";
  if (role === "assistant") {
    inner += `<div class="message-sender">Claude</div>`;
  } else if (role === "system") {
    inner += `<div class="message-sender">System</div>`;
  }
  inner += `<div class="message-content">${escapeHtml(content)}</div>`;

  msgEl.innerHTML = inner;
  messagesEl.appendChild(msgEl);
  scrollToBottom();

  // Track history (system messages don't count toward context)
  if (role !== "system") {
    messageHistory.push({ role, content });
  }
  updateContextBadge();
}

function showTypingIndicator() {
  const typingEl = document.createElement("div");
  typingEl.className = "message assistant typing-message";
  typingEl.innerHTML = `
    <div class="message-content">
      <div class="typing-indicator">
        <span></span><span></span><span></span>
      </div>
    </div>
  `;
  messagesEl.appendChild(typingEl);
  scrollToBottom();
}

// ========================================
// UI Helpers
// ========================================
function setProcessing(state) {
  isProcessing = state;
  sendBtnEl.disabled = state;
  if (state) {
    showTypingIndicator();
  }
}

function scrollToBottom() {
  messagesEl.scrollTop = messagesEl.scrollHeight;
}

function autoResizeTextarea() {
  chatInputEl.style.height = "auto";
  chatInputEl.style.height = Math.min(chatInputEl.scrollHeight, 120) + "px";
}

function escapeHtml(text) {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

function updateContextBadge(stats) {
  const pricing = MODEL_PRICING[currentModel] || MODEL_PRICING["claude-sonnet-4-5-20250929"];
  const contextWindow = pricing.contextWindow;

  let percent = 0;
  let costText = "$0.00";
  let inputTokens = 0;

  if (stats && stats.last_input_tokens > 0) {
    inputTokens = stats.last_input_tokens;
    percent = Math.min(Math.round((inputTokens / contextWindow) * 100), 100);

    // Calculate session cost
    const inputCost = (stats.total_input_tokens / 1_000_000) * pricing.input;
    const outputCost = (stats.total_output_tokens / 1_000_000) * pricing.output;
    const totalCost = inputCost + outputCost;
    costText = `$${totalCost.toFixed(4)}`;
  }

  // Update badge display
  const textEl = contextBadgeEl.querySelector(".context-text");
  textEl.textContent = `${percent}%`;

  // Update cost display
  const costEl = document.getElementById("cost-badge");
  if (costEl) {
    costEl.textContent = costText;
  }

  // Update token detail tooltip
  if (stats) {
    contextBadgeEl.title = `Context: ${inputTokens.toLocaleString()} / ${contextWindow.toLocaleString()} tokens\n` +
      `Session: ${stats.total_input_tokens.toLocaleString()} in / ${stats.total_output_tokens.toLocaleString()} out\n` +
      `Requests: ${stats.request_count}\nCost: ${costText}`;
  }

  // Color coding
  if (percent >= CONTEXT_CRITICAL_PERCENT) {
    contextBadgeEl.style.color = "var(--danger)";
  } else if (percent >= CONTEXT_WARN_PERCENT) {
    contextBadgeEl.style.color = "#ffa94d";
  } else {
    contextBadgeEl.style.color = "var(--text-secondary)";
  }
}

function checkContextWarning(stats) {
  if (!stats || stats.last_input_tokens === 0) return;

  const pricing = MODEL_PRICING[currentModel] || MODEL_PRICING["claude-sonnet-4-5-20250929"];
  const percent = Math.round((stats.last_input_tokens / pricing.contextWindow) * 100);

  // Remove existing warning
  removeContextWarning();

  if (percent >= CONTEXT_CRITICAL_PERCENT) {
    showContextWarning(
      "⚠️ コンテキスト使用率が90%を超えました。New Chatで新しい会話を始めることを推奨します。",
      "critical"
    );
  } else if (percent >= CONTEXT_WARN_PERCENT) {
    showContextWarning(
      `⚡ コンテキスト使用率 ${percent}% — 会話が長くなっています。`,
      "warn"
    );
  }
}

function showContextWarning(text, level) {
  const warningEl = document.createElement("div");
  warningEl.className = `context-warning ${level}`;
  warningEl.id = "context-warning";
  warningEl.innerHTML = `<span>${text}</span>`;

  if (level === "critical") {
    const btn = document.createElement("button");
    btn.className = "warning-new-chat-btn";
    btn.textContent = "New Chat";
    btn.addEventListener("click", () => {
      document.getElementById("new-chat-btn").click();
    });
    warningEl.appendChild(btn);
  }

  // Insert before input area
  const inputArea = document.querySelector(".input-area");
  inputArea.parentNode.insertBefore(warningEl, inputArea);
  scrollToBottom();
}

function removeContextWarning() {
  const existing = document.getElementById("context-warning");
  if (existing) existing.remove();
}

// ========================================
// Phase 3: Machine Status & Remote Exec
// ========================================

async function refreshMachineStatus() {
  try {
    // ポーリング中はdotをchecking状態に
    const dots = machineListEl.querySelectorAll(".status-dot");
    dots.forEach((d) => d.classList.add("checking"));

    const statuses = await invoke("get_machine_status");
    machineStatuses = statuses;
    renderMachineList(statuses);
  } catch (err) {
    console.error("Machine status poll error:", err);
  }
}

function renderMachineList(statuses) {
  machineListEl.innerHTML = "";
  for (const m of statuses) {
    const div = document.createElement("div");
    const isOnline = m.online;
    const isRemote = m.role !== "Commander";
    const isSelected = selectedRemoteMachine === m.name;

    div.className = `machine-item ${isOnline ? "online" : "offline"}${isRemote ? " selectable" : ""}${isSelected ? " selected" : ""}`;

    div.innerHTML = `
      <span class="status-dot"></span>
      <div class="machine-info">
        <span class="machine-name">${m.name}</span>
        <span class="machine-role">${m.role}</span>
      </div>`;

    if (isRemote) {
      div.addEventListener("click", () => selectRemoteMachine(m.name, isOnline));
    }

    machineListEl.appendChild(div);
  }
}

function selectRemoteMachine(name, isOnline) {
  if (selectedRemoteMachine === name) {
    // 同じマシンをクリック→選択解除
    selectedRemoteMachine = null;
    remotePanelEl.style.display = "none";
    remoteOutputEl.textContent = "";
  } else {
    selectedRemoteMachine = name;
    remotePanelEl.style.display = "block";
    remoteTargetLabel.textContent = `${name}${isOnline ? "" : " (offline)"}`;
    remoteOutputEl.textContent = "";
    remoteCmdInput.value = "";
    remoteCmdInput.focus();
  }
  // リスト再描画で選択状態を反映
  renderMachineList(machineStatuses);
}

async function handleRemoteExec() {
  if (!selectedRemoteMachine) return;
  const cmd = remoteCmdInput.value.trim();
  if (!cmd) return;

  remoteExecBtn.disabled = true;
  remoteOutputEl.innerHTML = '<span style="color:var(--text-muted)">実行中...</span>';

  try {
    const result = await invoke("execute_remote_command", {
      machineName: selectedRemoteMachine,
      command: cmd,
    });

    let html = "";
    if (result.stdout) {
      html += `<span class="cmd-success">${escapeHtml(result.stdout)}</span>`;
    }
    if (result.stderr) {
      html += `<span class="cmd-error">${escapeHtml(result.stderr)}</span>`;
    }
    if (!result.stdout && !result.stderr) {
      html = `<span class="cmd-success">exit: ${result.exit_code}</span>`;
    }
    remoteOutputEl.innerHTML = html;
  } catch (err) {
    remoteOutputEl.innerHTML = `<span class="cmd-error">${escapeHtml(String(err))}</span>`;
  } finally {
    remoteExecBtn.disabled = false;
    remoteCmdInput.value = "";
    remoteCmdInput.focus();
  }
}

function escapeHtml(text) {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}
