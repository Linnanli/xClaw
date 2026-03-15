// IronClaw Desktop Client - Tauri Integration

// Debug: Log when script loads
console.log('app-desktop.js loaded');

// Helper function to invoke Tauri commands with error handling
// Tauri 2.0 uses __TAURI_INTERNALS__ for IPC
async function invokeTauri(command, args = {}) {
  // Wait for Tauri internals to be available (with timeout)
  let attempts = 0;
  while (typeof window.__TAURI_INTERNALS__ === 'undefined' && attempts < 50) {
    await new Promise(resolve => setTimeout(resolve, 100));
    attempts++;
  }
  
  console.log(`invokeTauri('${command}') - Tauri internals available:`, typeof window.__TAURI_INTERNALS__ !== 'undefined', 'attempts:', attempts);
  
  if (typeof window.__TAURI_INTERNALS__ === 'undefined') {
    throw new Error('Tauri API not available. Make sure you are running this in a Tauri application.');
  }
  
  // Use Tauri 2.0's invoke method
  return await window.__TAURI_INTERNALS__.invoke(command, args);
}

let currentThreadId = null;
let currentTab = 'chat';
let approvalPending = null;
let cotPending = null;

// --- Authentication ---

async function initializeAuth() {
  try {
    // Check if master password is already set
    const sessionId = sessionStorage.getItem('session_id');
    if (sessionId) {
      // Already authenticated
      document.getElementById('auth-screen').style.display = 'none';
      document.getElementById('app').style.display = 'flex';
      initializeApp();
      return;
    }

    // Check if we need to setup master password
    const setupStatus = await invokeTauri('check_setup_status');
    if (!setupStatus.password_set) {
      // Show setup screen
      document.getElementById('auth-setup').style.display = 'block';
      document.getElementById('auth-login').style.display = 'none';
      setupPasswordInputListeners();
    } else {
      // Show login screen
      document.getElementById('auth-setup').style.display = 'none';
      document.getElementById('auth-login').style.display = 'block';
    }
  } catch (err) {
    console.error('Failed to initialize auth:', err);
    // Default to login screen
    document.getElementById('auth-setup').style.display = 'none';
    document.getElementById('auth-login').style.display = 'block';
  }
}

function setupPasswordInputListeners() {
  const setupPassword = document.getElementById('setup-password');
  const confirmPassword = document.getElementById('setup-password-confirm');
  
  setupPassword.addEventListener('input', updatePasswordStrength);
  confirmPassword.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') setupMasterPassword();
  });
}

function updatePasswordStrength() {
  const password = document.getElementById('setup-password').value;
  const strengthDiv = document.getElementById('password-strength');
  
  if (!password) {
    strengthDiv.innerHTML = '';
    return;
  }
  
  let strength = 0;
  let feedback = [];
  
  if (password.length >= 12) strength++;
  else feedback.push('至少 12 个字符');
  
  if (/[a-z]/.test(password)) strength++;
  else feedback.push('小写字母');
  
  if (/[A-Z]/.test(password)) strength++;
  else feedback.push('大写字母');
  
  if (/[0-9]/.test(password)) strength++;
  else feedback.push('数字');
  
  if (/[^a-zA-Z0-9]/.test(password)) strength++;
  else feedback.push('特殊字符');
  
  let strengthText = '';
  let strengthClass = '';
  
  if (strength < 3) {
    strengthText = '弱';
    strengthClass = 'weak';
  } else if (strength < 4) {
    strengthText = '中等';
    strengthClass = 'fair';
  } else {
    strengthText = '强';
    strengthClass = 'strong';
  }
  
  strengthDiv.innerHTML = `
    <div class="strength-bar">
      <div class="strength-fill ${strengthClass}" style="width: ${(strength / 5) * 100}%"></div>
    </div>
    <div class="strength-text ${strengthClass}">${strengthText}</div>
    ${feedback.length > 0 ? `<div class="strength-feedback">缺少：${feedback.join('、')}</div>` : ''}
  `;
}

async function setupMasterPassword() {
  const password = document.getElementById('setup-password').value;
  const confirmPassword = document.getElementById('setup-password-confirm').value;
  const errorDiv = document.getElementById('setup-error');
  
  errorDiv.textContent = '';
  
  if (!password || !confirmPassword) {
    errorDiv.textContent = '请填写所有字段';
    return;
  }
  
  if (password !== confirmPassword) {
    errorDiv.textContent = '密码不匹配';
    return;
  }
  
  try {
    const result = await invokeTauri('setup_master_password', { password });
    
    if (result.success) {
      // Switch to login screen
      document.getElementById('auth-setup').style.display = 'none';
      document.getElementById('auth-login').style.display = 'block';
      document.getElementById('master-password').focus();
      showToast('主密码设置成功', 'success');
    } else {
      errorDiv.textContent = result.message || '设置密码失败';
    }
  } catch (err) {
    errorDiv.textContent = '错误：' + err.message;
    console.error('Setup error:', err);
  }
}

async function authenticateDesktop() {
  const password = document.getElementById('master-password').value.trim();
  if (!password) {
    document.getElementById('auth-error').textContent = '请输入密码';
    return;
  }

  try {
    // Call Tauri command to unlock the app
    const result = await invokeTauri('unlock_app', { password });
    
    if (result.success) {
      sessionStorage.setItem('session_id', result.session_id);
      document.getElementById('auth-screen').style.display = 'none';
      document.getElementById('app').style.display = 'flex';
      initializeApp();
    } else {
      document.getElementById('auth-error').textContent = result.message || '认证失败';
    }
  } catch (err) {
    document.getElementById('auth-error').textContent = '错误：' + err.message;
    console.error('Authentication error:', err);
  }
}

document.getElementById('master-password')?.addEventListener('keydown', (e) => {
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
      
      // Load data for specific tabs
      if (tab === 'extensions') {
        loadExtensions();
      } else if (tab === 'routines') {
        loadRoutines();
      } else if (tab === 'plugins') {
        loadPlugins();
      } else if (tab === 'skills') {
        loadSkills();
      }
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
    alert('请先选择或创建一个对话');
    return;
  }

  addMessage('user', content);
  input.value = '';
  autoResizeTextarea(input);

  try {
    // Send message via Tauri command
    await invokeTauri('send_message', {
      thread_id: currentThreadId,
      content: content
    });
  } catch (err) {
    addMessage('system', '错误：' + err.message);
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
    const threads = await invokeTauri('get_threads');
    const list = document.getElementById('thread-list');
    list.innerHTML = '';
    
    threads.forEach(thread => {
      const item = document.createElement('div');
      item.className = 'thread-item';
      if (thread.id === currentThreadId) item.classList.add('active');
      
      item.textContent = thread.title || '未命名';
      item.addEventListener('click', () => switchThread(thread.id));
      
      list.appendChild(item);
    });
  } catch (err) {
    console.error('加载对话失败：', err);
  }
}

async function createNewThread() {
  try {
    const thread = await invokeTauri('create_thread');
    currentThreadId = thread.id;
    document.getElementById('chat-messages').innerHTML = '';
    loadThreads();
  } catch (err) {
    alert('创建对话失败：' + err.message);
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
    await invokeTauri('approve_operation', {
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
    await invokeTauri('deny_operation', {
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
  initializeAuth();
});


// --- Plugin Management ---

let currentPluginTab = 'installed';
let installedPlugins = [];
let availablePlugins = [];
let pendingUpdates = [];
let selectedPlugin = null;

async function loadPlugins() {
  try {
    installedPlugins = await invokeTauri('get_installed_plugins');
    availablePlugins = await invokeTauri('get_available_plugins');
    pendingUpdates = await invokeTauri('check_plugin_updates');
    
    renderPluginsList();
  } catch (err) {
    showToast('加载插件失败：' + err.message, 'error');
  }
}

function switchPluginTab(tab) {
  currentPluginTab = tab;
  
  document.querySelectorAll('.plugin-tab-btn').forEach(btn => {
    btn.classList.remove('active');
  });
  event.target.classList.add('active');
  
  document.querySelectorAll('.plugin-panel').forEach(panel => {
    panel.classList.remove('active');
  });
  document.getElementById(tab + '-plugins-panel').classList.add('active');
  
  renderPluginsList();
}

function renderPluginsList() {
  let plugins = [];
  let containerId = '';
  
  if (currentPluginTab === 'installed') {
    plugins = installedPlugins;
    containerId = 'installed-plugins-list';
  } else if (currentPluginTab === 'available') {
    plugins = availablePlugins;
    containerId = 'available-plugins-list';
  } else if (currentPluginTab === 'updates') {
    plugins = pendingUpdates;
    containerId = 'updates-plugins-list';
  }
  
  const container = document.getElementById(containerId);
  container.innerHTML = '';
  
  if (plugins.length === 0) {
    container.innerHTML = '<p style="text-align: center; color: #999; padding: 40px;">未找到插件</p>';
    return;
  }
  
  plugins.forEach(plugin => {
    const card = createPluginCard(plugin);
    container.appendChild(card);
  });
}

function createPluginCard(plugin) {
  const card = document.createElement('div');
  card.className = 'plugin-card';
  
  let version = plugin.version || plugin.new_version || 'N/A';
  let title = plugin.name || plugin.plugin_id || 'Unknown';
  let description = plugin.description || plugin.changelog || 'No description';
  let author = plugin.author || 'Unknown';
  
  let statusBadges = '';
  if (currentPluginTab === 'installed') {
    statusBadges = plugin.enabled ? 
      '<span class="plugin-status-badge">已启用</span>' :
      '<span class="plugin-status-badge disabled">已禁用</span>';
  } else if (currentPluginTab === 'updates') {
    statusBadges = '<span class="plugin-status-badge update">有可用更新</span>';
  }
  
  let actions = '';
  if (currentPluginTab === 'installed') {
    actions = `
      <button class="plugin-action" onclick="togglePluginStatus('${plugin.metadata?.id || plugin.id}', ${plugin.enabled})">
        ${plugin.enabled ? '禁用' : '启用'}
      </button>
      <button class="plugin-action" onclick="uninstallPlugin('${plugin.metadata?.id || plugin.id}')">卸载</button>
    `;
  } else if (currentPluginTab === 'available') {
    actions = `
      <button class="plugin-action primary" onclick="installPlugin('${plugin.id}')">安装</button>
    `;
  } else if (currentPluginTab === 'updates') {
    actions = `
      <button class="plugin-action primary" onclick="updatePlugin('${plugin.plugin_id}')">更新</button>
    `;
  }
  
  card.innerHTML = `
    <div class="plugin-card-header">
      <div class="plugin-card-title">${escapeHtml(title)}</div>
      <div class="plugin-card-version">${escapeHtml(version)}</div>
    </div>
    <div class="plugin-card-author">${escapeHtml(author)}</div>
    <div class="plugin-card-description">${escapeHtml(description)}</div>
    <div class="plugin-card-status">${statusBadges}</div>
    <div class="plugin-card-actions">${actions}</div>
  `;
  
  card.addEventListener('click', () => showPluginDetails(plugin));
  
  return card;
}

function showPluginDetails(plugin) {
  selectedPlugin = plugin;
  const modal = document.getElementById('plugin-modal');
  const title = document.getElementById('plugin-modal-title');
  const details = document.getElementById('plugin-modal-details');
  const actionBtn = document.getElementById('plugin-action-btn');
  
  let pluginName = plugin.name || plugin.plugin_id || 'Unknown';
  let pluginVersion = plugin.version || plugin.new_version || 'N/A';
  let pluginAuthor = plugin.author || 'Unknown';
  let pluginDescription = plugin.description || plugin.changelog || 'No description';
  
  title.textContent = pluginName;
  
  let detailsHtml = `
    <div class="plugin-detail-section">
      <div class="plugin-detail-label">Version</div>
      <div class="plugin-detail-value">${escapeHtml(pluginVersion)}</div>
    </div>
    <div class="plugin-detail-section">
      <div class="plugin-detail-label">Author</div>
      <div class="plugin-detail-value">${escapeHtml(pluginAuthor)}</div>
    </div>
    <div class="plugin-detail-section">
      <div class="plugin-detail-label">Description</div>
      <div class="plugin-detail-value">${escapeHtml(pluginDescription)}</div>
    </div>
  `;
  
  if (currentPluginTab === 'installed' && plugin.metadata?.resource_requirements) {
    const reqs = plugin.metadata.resource_requirements;
    detailsHtml += `
      <div class="plugin-detail-section">
        <div class="plugin-detail-label">Resource Requirements</div>
        <div class="plugin-detail-value">
          Memory: ${reqs.min_memory_mb}MB<br>
          Disk: ${reqs.min_disk_mb}MB
        </div>
      </div>
    `;
  }
  
  details.innerHTML = detailsHtml;
  
  if (currentPluginTab === 'installed') {
    actionBtn.textContent = plugin.enabled ? 'Disable' : 'Enable';
    actionBtn.onclick = () => togglePluginStatus(plugin.metadata?.id || plugin.id, plugin.enabled);
  } else if (currentPluginTab === 'available') {
    actionBtn.textContent = 'Install';
    actionBtn.onclick = () => installPlugin(plugin.id);
  } else if (currentPluginTab === 'updates') {
    actionBtn.textContent = 'Update';
    actionBtn.onclick = () => updatePlugin(plugin.plugin_id);
  }
  
  modal.style.display = 'flex';
}

function closePluginModal() {
  document.getElementById('plugin-modal').style.display = 'none';
  selectedPlugin = null;
}

async function installPlugin(pluginId) {
  try {
    await invokeTauri('install_plugin', { plugin_id: pluginId });
    showToast('Plugin installed successfully', 'success');
    closePluginModal();
    loadPlugins();
  } catch (err) {
    showToast('Failed to install plugin: ' + err.message, 'error');
  }
}

async function uninstallPlugin(pluginId) {
  if (!confirm('Are you sure you want to uninstall this plugin?')) return;
  
  try {
    await invokeTauri('uninstall_plugin', { plugin_id: pluginId });
    showToast('Plugin uninstalled successfully', 'success');
    closePluginModal();
    loadPlugins();
  } catch (err) {
    showToast('Failed to uninstall plugin: ' + err.message, 'error');
  }
}

async function togglePluginStatus(pluginId, currentlyEnabled) {
  try {
    if (currentlyEnabled) {
      await invokeTauri('disable_plugin', { plugin_id: pluginId });
      showToast('Plugin disabled', 'success');
    } else {
      await invokeTauri('enable_plugin', { plugin_id: pluginId });
      showToast('Plugin enabled', 'success');
    }
    closePluginModal();
    loadPlugins();
  } catch (err) {
    showToast('Failed to toggle plugin: ' + err.message, 'error');
  }
}

async function updatePlugin(pluginId) {
  try {
    await invokeTauri('update_plugin', { plugin_id: pluginId });
    showToast('Plugin updated successfully', 'success');
    closePluginModal();
    loadPlugins();
  } catch (err) {
    showToast('Failed to update plugin: ' + err.message, 'error');
  }
}

async function refreshPlugins() {
  showToast('Refreshing plugins...', 'info');
  await loadPlugins();
  showToast('Plugins refreshed', 'success');
}

function performPluginAction() {
  // This is called by the modal action button
  // The actual action is set in showPluginDetails
}

// --- Offline Mode ---

let offlineMode = false;

async function initializeOfflineMode() {
  try {
    const state = await invokeTauri('get_offline_state');
    offlineMode = state.is_offline;
    updateOfflineIndicator();
  } catch (err) {
    console.error('Failed to initialize offline mode:', err);
  }
}

function updateOfflineIndicator() {
  const indicator = document.getElementById('offline-indicator');
  const status = document.getElementById('sse-status');
  const statusDiv = document.getElementById('gateway-status-trigger');
  
  if (offlineMode) {
    indicator.style.display = 'inline-block';
    status.textContent = 'Offline';
    statusDiv.classList.add('offline');
  } else {
    indicator.style.display = 'none';
    status.textContent = 'Connected';
    statusDiv.classList.remove('offline');
  }
}

async function toggleOfflineMode() {
  try {
    if (offlineMode) {
      await invokeTauri('disable_offline_mode');
      offlineMode = false;
      showToast('Offline mode disabled', 'success');
    } else {
      await invokeTauri('enable_offline_mode');
      offlineMode = true;
      showToast('Offline mode enabled', 'info');
    }
    updateOfflineIndicator();
  } catch (err) {
    showToast('Failed to toggle offline mode: ' + err.message, 'error');
  }
}

// Update initialization to include offline mode and plugins
const originalInitializeApp = initializeApp;
initializeApp = function() {
  originalInitializeApp();
  initializeOfflineMode();
  
  // Load plugins when plugins tab is clicked
  document.querySelectorAll('.tab-bar button').forEach(btn => {
    if (btn.dataset.tab === 'plugins') {
      btn.addEventListener('click', loadPlugins);
    }
  });
};


// Extensions Management
let installedExtensionIds = [];

async function loadExtensions() {
  try {
    const installed = await invokeTauri('get_installed_extensions');
    const available = await invokeTauri('get_available_extensions');
    
    // Store installed extension IDs for quick lookup
    installedExtensionIds = installed.map(ext => ext.metadata.id);
    
    renderInstalledExtensions(installed);
    renderAvailableExtensions(available);
  } catch (error) {
    console.error('Failed to load extensions:', error);
    showToast('Failed to load extensions', 'error');
  }
}

function renderInstalledExtensions(extensions) {
  const list = document.getElementById('installed-extensions-list');
  list.innerHTML = '';
  
  if (extensions.length === 0) {
    list.innerHTML = '<p class="empty-state">No extensions installed</p>';
    return;
  }
  
  extensions.forEach(ext => {
    const card = document.createElement('div');
    card.className = 'extension-card';
    card.innerHTML = `
      <div class="extension-header">
        <h3>${ext.metadata.name}</h3>
        <span class="extension-version">${ext.metadata.version}</span>
      </div>
      <p class="extension-description">${ext.metadata.description}</p>
      <p class="extension-author">by ${ext.metadata.author}</p>
      <div class="extension-tools">
        <strong>Tools:</strong> ${ext.metadata.tools.join(', ') || 'None'}
      </div>
      <div class="extension-actions">
        <button onclick="toggleExtensionStatus('${ext.metadata.id}', ${ext.enabled})" class="btn-toggle">
          ${ext.enabled ? 'Disable' : 'Enable'}
        </button>
        <button onclick="uninstallExtensionAction('${ext.metadata.id}')" class="btn-uninstall">Uninstall</button>
      </div>
    `;
    list.appendChild(card);
  });
}

function renderAvailableExtensions(extensions) {
  const list = document.getElementById('available-extensions-list');
  list.innerHTML = '';
  
  if (extensions.length === 0) {
    list.innerHTML = '<p class="empty-state">No available extensions</p>';
    return;
  }
  
  extensions.forEach(ext => {
    const isInstalled = installedExtensionIds.includes(ext.id);
    const card = document.createElement('div');
    card.className = 'extension-card';
    card.innerHTML = `
      <div class="extension-header">
        <h3>${ext.name}</h3>
        <span class="extension-version">${ext.version}</span>
      </div>
      <p class="extension-description">${ext.description}</p>
      <p class="extension-author">by ${ext.author}</p>
      <div class="extension-tools">
        <strong>Tools:</strong> ${ext.tools.join(', ') || 'None'}
      </div>
      <div class="extension-actions">
        <button onclick="installExtensionAction('${ext.id}')" class="btn-install" ${isInstalled ? 'disabled' : ''}>
          ${isInstalled ? 'Already Installed' : 'Install'}
        </button>
      </div>
    `;
    list.appendChild(card);
  });
}

function switchExtensionTab(tab) {
  document.querySelectorAll('.extension-tab-btn').forEach(btn => btn.classList.remove('active'));
  document.querySelectorAll('.extension-panel').forEach(panel => panel.classList.remove('active'));
  
  // Find the button for this tab and mark it as active
  const buttons = document.querySelectorAll('.extension-tab-btn');
  buttons.forEach(btn => {
    if (btn.textContent.includes(tab === 'installed' ? '已安装' : '可用')) {
      btn.classList.add('active');
    }
  });
  
  document.getElementById(`${tab}-extensions-panel`).classList.add('active');
}

async function installExtensionAction(extensionId) {
  try {
    const available = await invokeTauri('get_available_extensions');
    const ext = available.find(e => e.id === extensionId);
    if (ext) {
      await invokeTauri('install_extension', { metadata: ext });
      showToast(`扩展 ${ext.name} 已安装`, 'success');
      // 重新加载扩展列表以更新 UI
      await loadExtensions();
      // 切换到已安装标签页以显示新安装的扩展
      switchExtensionTab('installed');
    }
  } catch (error) {
    console.error('Failed to install extension:', error);
    showToast('安装扩展失败', 'error');
  }
}

async function uninstallExtensionAction(extensionId) {
  if (confirm('确定要卸载此扩展吗？')) {
    try {
      await invokeTauri('uninstall_extension', { extensionId });
      showToast('扩展已卸载', 'success');
      await loadExtensions();
      switchExtensionTab('available');
    } catch (error) {
      console.error('Failed to uninstall extension:', error);
      showToast('卸载扩展失败', 'error');
    }
  }
}

async function toggleExtensionStatus(extensionId, currentlyEnabled) {
  try {
    if (currentlyEnabled) {
      await invokeTauri('disable_extension', { extensionId });
      showToast('扩展已禁用', 'success');
    } else {
      await invokeTauri('enable_extension', { extensionId });
      showToast('扩展已启用', 'success');
    }
    await loadExtensions();
  } catch (error) {
    console.error('Failed to toggle extension:', error);
    showToast('切换扩展状态失败', 'error');
  }
}

async function refreshExtensions() {
  loadExtensions();
}

// Routines Management
async function loadRoutines() {
  try {
    const routines = await invokeTauri('get_routines');
    renderRoutines(routines);
  } catch (error) {
    console.error('Failed to load routines:', error);
    showToast('Failed to load routines', 'error');
  }
}

function renderRoutines(routines) {
  const list = document.getElementById('routines-list');
  list.innerHTML = '';
  
  if (routines.length === 0) {
    list.innerHTML = '<p class="empty-state">No routines created</p>';
    return;
  }
  
  routines.forEach(routine => {
    const card = document.createElement('div');
    card.className = 'routine-card';
    const statusClass = routine.status.toLowerCase();
    card.innerHTML = `
      <div class="routine-header">
        <h3>${routine.name}</h3>
        <span class="routine-status ${statusClass}">${routine.status}</span>
      </div>
      <p class="routine-description">${routine.description}</p>
      <div class="routine-trigger">
        <strong>Trigger:</strong> ${JSON.stringify(routine.trigger).replace(/"/g, '')}
      </div>
      <div class="routine-actions">
        <button onclick="triggerRoutineAction('${routine.id}')" class="btn-trigger">Trigger Now</button>
        <button onclick="toggleRoutineStatus('${routine.id}', '${routine.status}')" class="btn-toggle">
          ${routine.status === 'Active' ? 'Pause' : routine.status === 'Paused' ? 'Resume' : 'Enable'}
        </button>
        <button onclick="deleteRoutineAction('${routine.id}')" class="btn-delete">Delete</button>
      </div>
    `;
    list.appendChild(card);
  });
}

function showCreateRoutineModal() {
  document.getElementById('routine-modal').style.display = 'flex';
  document.getElementById('routine-trigger').value = 'manual';
  document.getElementById('trigger-value-group').style.display = 'none';
}

function closeRoutineModal() {
  document.getElementById('routine-modal').style.display = 'none';
  document.getElementById('routine-name').value = '';
  document.getElementById('routine-description').value = '';
}

document.addEventListener('DOMContentLoaded', function() {
  const triggerSelect = document.getElementById('routine-trigger');
  if (triggerSelect) {
    triggerSelect.addEventListener('change', function() {
      const valueGroup = document.getElementById('trigger-value-group');
      if (this.value === 'manual') {
        valueGroup.style.display = 'none';
      } else {
        valueGroup.style.display = 'block';
      }
    });
  }
});

async function createNewRoutine() {
  const name = document.getElementById('routine-name').value;
  const description = document.getElementById('routine-description').value;
  const triggerType = document.getElementById('routine-trigger').value;
  const triggerValue = document.getElementById('routine-trigger-value').value;
  
  if (!name || !description) {
    showToast('Please fill in all required fields', 'error');
    return;
  }
  
  try {
    let trigger;
    if (triggerType === 'manual') {
      trigger = { Manual: null };
    } else if (triggerType === 'time') {
      trigger = { Time: triggerValue || '0 9 * * *' };
    } else {
      trigger = { Event: triggerValue || 'default_event' };
    }
    
    await invokeTauri('create_routine', {
      name,
      description,
      trigger,
      actions: []
    });
    
    showToast('Routine created successfully', 'success');
    closeRoutineModal();
    loadRoutines();
  } catch (error) {
    console.error('Failed to create routine:', error);
    showToast('Failed to create routine', 'error');
  }
}

async function triggerRoutineAction(routineId) {
  try {
    await invokeTauri('trigger_routine', { routineId });
    showToast('Routine triggered', 'success');
    loadRoutines();
  } catch (error) {
    console.error('Failed to trigger routine:', error);
    showToast('Failed to trigger routine', 'error');
  }
}

async function toggleRoutineStatus(routineId, currentStatus) {
  try {
    if (currentStatus === 'Active') {
      await invokeTauri('pause_routine', { routineId });
    } else if (currentStatus === 'Paused') {
      await invokeTauri('enable_routine', { routineId });
    } else {
      await invokeTauri('enable_routine', { routineId });
    }
    loadRoutines();
  } catch (error) {
    console.error('Failed to toggle routine:', error);
    showToast('Failed to toggle routine', 'error');
  }
}

async function deleteRoutineAction(routineId) {
  if (confirm('Are you sure you want to delete this routine?')) {
    try {
      await invokeTauri('delete_routine', { routineId });
      showToast('Routine deleted', 'success');
      loadRoutines();
    } catch (error) {
      console.error('Failed to delete routine:', error);
      showToast('Failed to delete routine', 'error');
    }
  }
}

// Skills Management
let installedSkillIds = [];

async function loadSkills() {
  try {
    const skills = await invokeTauri('get_available_skills');
    const installed = await invokeTauri('get_installed_skills');
    
    // Store installed skill IDs for quick lookup
    installedSkillIds = installed.map(skill => skill.name);
    
    renderAvailableSkills(skills);
    renderInstalledSkills(installed);
  } catch (error) {
    console.error('Failed to load skills:', error);
    showToast('Failed to load skills', 'error');
  }
}

function renderAvailableSkills(skills) {
  const list = document.getElementById('available-skills-list');
  list.innerHTML = '';
  
  if (skills.length === 0) {
    list.innerHTML = '<p class="empty-state">No available skills</p>';
    return;
  }
  
  skills.forEach(skill => {
    const isInstalled = installedSkillIds.includes(skill.name);
    const card = document.createElement('div');
    card.className = 'skill-card';
    
    const keywordsHtml = skill.keywords && skill.keywords.length > 0 
      ? `<div class="skill-keywords">${skill.keywords.map(k => `<span class="keyword-tag">${k}</span>`).join('')}</div>`
      : '';
    
    card.innerHTML = `
      <div class="skill-header">
        <h3>${skill.name}</h3>
        <span class="skill-version">${skill.version}</span>
      </div>
      <p class="skill-description">${skill.description}</p>
      <div class="skill-meta">
        <span class="skill-trust">Trust: ${skill.trust}</span>
        <span class="skill-source">${skill.source}</span>
      </div>
      ${keywordsHtml}
      <div class="skill-actions">
        <button onclick="installSkillAction('${skill.name}')" class="btn-activate" ${isInstalled ? 'disabled' : ''}>
          ${isInstalled ? 'Installed' : 'Install'}
        </button>
      </div>
    `;
    list.appendChild(card);
  });
}

function renderInstalledSkills(skills) {
  const list = document.getElementById('installed-skills-list');
  list.innerHTML = '';
  
  if (skills.length === 0) {
    list.innerHTML = '<p class="empty-state">No skills installed</p>';
    return;
  }
  
  skills.forEach(skill => {
    const card = document.createElement('div');
    card.className = 'skill-card';
    
    const keywordsHtml = skill.keywords && skill.keywords.length > 0 
      ? `<div class="skill-keywords">${skill.keywords.map(k => `<span class="keyword-tag">${k}</span>`).join('')}</div>`
      : '';
    
    card.innerHTML = `
      <div class="skill-header">
        <h3>${skill.name}</h3>
        <span class="skill-version">${skill.version}</span>
      </div>
      <p class="skill-description">${skill.description}</p>
      <div class="skill-meta">
        <span class="skill-trust">Trust: ${skill.trust}</span>
        <span class="skill-source">${skill.source}</span>
      </div>
      ${keywordsHtml}
      <div class="skill-actions">
        <button onclick="uninstallSkillAction('${skill.name}')" class="btn-deactivate">Uninstall</button>
      </div>
    `;
    list.appendChild(card);
  });
}

function switchSkillTab(tab) {
  document.querySelectorAll('.skill-tab-btn').forEach(btn => btn.classList.remove('active'));
  document.querySelectorAll('.skill-panel').forEach(panel => panel.classList.remove('active'));
  
  const buttons = document.querySelectorAll('.skill-tab-btn');
  buttons.forEach(btn => {
    if (btn.textContent.includes(tab === 'installed' ? '已安装' : '可用技能')) {
      btn.classList.add('active');
    }
  });
  
  document.getElementById(`${tab}-skills-panel`).classList.add('active');
}

async function installSkillAction(skillName) {
  try {
    await invokeTauri('install_skill', { name: skillName });
    showToast(`Skill "${skillName}" installed`, 'success');
    loadSkills();
    switchSkillTab('installed');
  } catch (error) {
    console.error('Failed to install skill:', error);
    showToast('Failed to install skill', 'error');
  }
}

async function uninstallSkillAction(skillName) {
  if (confirm(`Are you sure you want to uninstall "${skillName}"?`)) {
    try {
      await invokeTauri('uninstall_skill', { name: skillName });
      showToast(`Skill "${skillName}" uninstalled`, 'success');
      loadSkills();
    } catch (error) {
      console.error('Failed to uninstall skill:', error);
      showToast('Failed to uninstall skill', 'error');
    }
  }
}

function refreshSkills() {
  loadSkills();
  showToast('Skills refreshed', 'success');
}
