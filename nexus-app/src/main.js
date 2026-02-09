// ========================================
// Project Nexus — Chat UI Controller
// Phase 2: Anthropic API integration
// ========================================

const { invoke } = window.__TAURI__.core;

// DOM Elements
let messagesEl;
let chatInputEl;
let chatFormEl;
let sendBtnEl;
let contextBadgeEl;

// State
let isProcessing = false;
let messageHistory = [];

// ========================================
// Initialization
// ========================================
window.addEventListener("DOMContentLoaded", () => {
  messagesEl = document.getElementById("messages");
  chatInputEl = document.getElementById("chat-input");
  chatFormEl = document.getElementById("chat-form");
  sendBtnEl = document.getElementById("send-btn");
  contextBadgeEl = document.getElementById("context-badge");

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
    // Load current model
    invoke("get_current_model").then((model) => {
      modelSelect.value = model;
    });

    modelSelect.addEventListener("change", async (e) => {
      try {
        const result = await invoke("set_model", { modelId: e.target.value });
        addMessage("system", result);
      } catch (err) {
        addMessage("system", `Error: ${err}`);
        // Revert selector
        const current = await invoke("get_current_model");
        modelSelect.value = current;
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
        updateContextBadge();
        addMessage("system", "会話履歴をクリアしました");
      } catch (err) {
        addMessage("system", `Error: ${err}`);
      }
    });
  }

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
    addMessage("assistant", response);
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

function updateContextBadge() {
  const totalChars = messageHistory.reduce((sum, m) => sum + m.content.length, 0);
  const estimatedTokens = Math.round(totalChars / 4);
  const maxTokens = 200000;
  const percent = Math.min(Math.round((estimatedTokens / maxTokens) * 100), 100);

  const textEl = contextBadgeEl.querySelector(".context-text");
  textEl.textContent = `${percent}%`;

  if (percent > 75) {
    contextBadgeEl.style.color = "var(--danger)";
  } else if (percent > 50) {
    contextBadgeEl.style.color = "#ffa94d";
  } else {
    contextBadgeEl.style.color = "var(--text-secondary)";
  }
}
