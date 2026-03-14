// IronClaw Desktop Client - Tauri Integration

let currentThreadId = null;
let currentTab = 'chat';
let approvalPending = null;
let cotPending = null;

// --- Authentication ---

async function authenticateDesktop() {
  const password = document.getElementById('master-password').value.trim();
  if (!password) {
    document.getElementById('auth-error').textContent = 'Password required';
    return;
  }

  try {
    // Call Tauri command to unlock the app
    const result = await window.__TAURI__.invoke('unlock_app', { password });
    
    if (result.success) {
      sessionStorage.setItem('session_id', result.session_id);
      document.getElementById('auth-screen').style.display = 'none';
      document.getElementById('app').style.display = 'flex';
      initializeApp();
    } else {
      document.getElementById('auth-error').textContent = result.message || 'Authentication failed';
    }
  } catch (err) {
    document.getElementById('auth-error').textContent = 'Error: ' + err.message;
  }
}

document.getElementById('master-password').addEventListener('keydown', (e) => {
  if (e.key === 'Enter') authenticateDesktop();
});

// --- App Initialization ---

function initializeApp() {
  setupTabNavigation();
  setupChatInput();
  setupWatermark();
  loadThreads();
}

function setupTabNavigation() {
  document.querySelectorAll('.tab-bar button').forEach(btn => {
    btn.addEventListener('click', () => {
      const tab = btn.dataset.tab;
      if (!tab) return;
      
      document.querySelectorAll('.tab-bar button').forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      
      document.querySelectorAll('.tab-panel').forEach(p => p.classList.remove('active'));
      const panel = document.getElementById('tab-' + tab);
      if (panel) panel.classList.add('active');
      
      currentTab = tab;
    });
  });
}

function setupChatInput() {
  const input = document.getElementById('chat-input');
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  });
  
  input.addEventListener('input', () => {
    autoResizeTextarea(input);
  });
}

function autoResizeTextarea(textarea) {
  textarea.style.height = 'auto';
  textarea.style.height = Math.min(textarea.scrollHeight, 200) + 'px';
}

// --- Chat Functions ---

async function sendMessage() {
  const input = document.getElementById('chat-input');
  const content = input.value.trim();
  
  if (!content) return;
  if (!currentThreadId) {
    alert('Please select or create a thread first');
    return;
  }

  addMessage('user', content);
  input.value = '';
  autoResizeTextarea(input);

  try {
    // Send message via Tauri command
    await window.__TAURI__.invoke('send_message', {
      thread_id: currentThreadId,
      content: content
    });
  } catch (err) {
    addMessage('system', 'Error: ' + err.message);
  }
}

function addMessage(role, content) {
  const container = document.getElementById('chat-messages');
  const div = document.createElement('div');
  div.className = 'message ' + role;
  
  if (role === 'user') {
    div.textContent = content;
  } else if (role === 'assistant') {
    div.innerHTML = renderMarkdown(content);
  } else {
    div.textContent = content;
  }
  
  container.appendChild(div);
  container.scrollTop = container.scrollHeight;
}

function renderMarkdown(text) {
  if (typeof marked !== 'undefined') {
    return marked.parse(text);
  }
  return escapeHtml(text);
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

// --- Thread Management ---

async function loadThreads() {
  try {
    const threads = await window.__TAURI__.invoke('get_threads');
    const list = document.getElementById('thread-list');
    list.innerHTML = '';
    
    threads.forEach(thread => {
      const item = document.createElement('div');
      item.className = 'thread-item';
      if (thread.id === currentThreadId) item.classList.add('active');
      
      item.textContent = thread.title || 'Untitled';
      item.addEventListener('click', () => switchThread(thread.id));
      
      list.appendChild(item);
    });
  } catch (err) {
    console.error('Failed to load threads:', err);
  }
}

async function createNewThread() {
  try {
    const thread = await window.__TAURI__.invoke('create_thread');
    currentThreadId = thread.id;
    document.getElementById('chat-messages').innerHTML = '';
    loadThreads();
  } catch (err) {
    alert('Failed to create thread: ' + err.message);
  }
}

function switchThread(threadId) {
  currentThreadId = threadId;
  document.getElementById('chat-messages').innerHTML = '';
  loadThreads();
}

function switchToAssistant() {
  currentThreadId = null;
  document.getElementById('chat-messages').innerHTML = '';
  loadThreads();
}

// --- Approval Modal ---

function showApprovalModal(operation, details) {
  approvalPending = { operation, details };
  const modal = document.getElementById('approval-modal');
  const detailsDiv = document.getElementById('approval-details');
  
  detailsDiv.innerHTML = `
    <div class="approval-operation">${escapeHtml(operation)}</div>
    <div class="approval-details">${escapeHtml(details)}</div>
  `;
  
  modal.style.display = 'flex';
}

function closeApprovalModal() {
  document.getElementById('approval-modal').style.display = 'none';
  approvalPending = null;
}

async function approveApproval() {
  if (!approvalPending) return;
  
  try {
    await window.__TAURI__.invoke('approve_operation', {
      operation: approvalPending.operation
    });
    closeApprovalModal();
  } catch (err) {
    alert('Error: ' + err.message);
  }
}

async function denyApproval() {
  if (!approvalPending) return;
  
  try {
    await window.__TAURI__.invoke('deny_operation', {
      operation: approvalPending.operation
    });
    closeApprovalModal();
  } catch (err) {
    alert('Error: ' + err.message);
  }
}

// --- CoT Display ---

function showCoTModal(thinking) {
  cotPending = thinking;
  const modal = document.getElementById('cot-modal');
  const content = document.getElementById('cot-content');
  
  content.innerHTML = renderMarkdown(thinking);
  modal.style.display = 'flex';
}

function closeCoTModal() {
  document.getElementById('cot-modal').style.display = 'none';
  cotPending = null;
}

// --- Watermark ---

function setupWatermark() {
  const watermark = document.getElementById('watermark');
  const username = 'User'; // Get from session
  const userId = '12345'; // Get from session
  
  watermark.textContent = `${username} (${userId})`;
  watermark.style.opacity = '0.15';
  watermark.style.transform = 'rotate(-45deg)';
}

// --- Utility Functions ---

function showToast(message, type = 'info') {
  const toasts = document.getElementById('toasts');
  const toast = document.createElement('div');
  toast.className = 'toast ' + type;
  toast.textContent = message;
  toasts.appendChild(toast);
  
  setTimeout(() => {
    toast.remove();
  }, 3000);
}

// Initialize on load
window.addEventListener('DOMContentLoaded', () => {
  // Check if already authenticated
  const sessionId = sessionStorage.getItem('session_id');
  if (sessionId) {
    document.getElementById('auth-screen').style.display = 'none';
    document.getElementById('app').style.display = 'flex';
    initializeApp();
  }
});
