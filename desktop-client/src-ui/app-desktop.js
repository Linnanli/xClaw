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
      
      // Load data for specific tabs
      if (tab === 'extensions') {
        loadExtensions();
      } else if (tab === 'routines') {
        loadRoutines();
      } else if (tab === 'plugins') {
        loadPlugins();
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


// --- Plugin Management ---

let currentPluginTab = 'installed';
let installedPlugins = [];
let availablePlugins = [];
let pendingUpdates = [];
let selectedPlugin = null;

async function loadPlugins() {
  try {
    installedPlugins = await window.__TAURI__.invoke('get_installed_plugins');
    availablePlugins = await window.__TAURI__.invoke('get_available_plugins');
    pendingUpdates = await window.__TAURI__.invoke('check_plugin_updates');
    
    renderPluginsList();
  } catch (err) {
    showToast('Failed to load plugins: ' + err.message, 'error');
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
    container.innerHTML = '<p style="text-align: center; color: #999; padding: 40px;">No plugins found</p>';
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
      '<span class="plugin-status-badge">Enabled</span>' :
      '<span class="plugin-status-badge disabled">Disabled</span>';
  } else if (currentPluginTab === 'updates') {
    statusBadges = '<span class="plugin-status-badge update">Update Available</span>';
  }
  
  let actions = '';
  if (currentPluginTab === 'installed') {
    actions = `
      <button class="plugin-action" onclick="togglePluginStatus('${plugin.metadata?.id || plugin.id}', ${plugin.enabled})">
        ${plugin.enabled ? 'Disable' : 'Enable'}
      </button>
      <button class="plugin-action" onclick="uninstallPlugin('${plugin.metadata?.id || plugin.id}')">Uninstall</button>
    `;
  } else if (currentPluginTab === 'available') {
    actions = `
      <button class="plugin-action primary" onclick="installPlugin('${plugin.id}')">Install</button>
    `;
  } else if (currentPluginTab === 'updates') {
    actions = `
      <button class="plugin-action primary" onclick="updatePlugin('${plugin.plugin_id}')">Update</button>
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
    await window.__TAURI__.invoke('install_plugin', { plugin_id: pluginId });
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
    await window.__TAURI__.invoke('uninstall_plugin', { plugin_id: pluginId });
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
      await window.__TAURI__.invoke('disable_plugin', { plugin_id: pluginId });
      showToast('Plugin disabled', 'success');
    } else {
      await window.__TAURI__.invoke('enable_plugin', { plugin_id: pluginId });
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
    await window.__TAURI__.invoke('update_plugin', { plugin_id: pluginId });
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
    const state = await window.__TAURI__.invoke('get_offline_state');
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
      await window.__TAURI__.invoke('disable_offline_mode');
      offlineMode = false;
      showToast('Offline mode disabled', 'success');
    } else {
      await window.__TAURI__.invoke('enable_offline_mode');
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
async function loadExtensions() {
  try {
    const installed = await window.__TAURI__.invoke('get_installed_extensions');
    const available = await window.__TAURI__.invoke('get_available_extensions');
    
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
        <button onclick="installExtensionAction('${ext.id}')" class="btn-install">Install</button>
      </div>
    `;
    list.appendChild(card);
  });
}

function switchExtensionTab(tab) {
  document.querySelectorAll('.extension-tab-btn').forEach(btn => btn.classList.remove('active'));
  document.querySelectorAll('.extension-panel').forEach(panel => panel.classList.remove('active'));
  
  event.target.classList.add('active');
  document.getElementById(`${tab}-extensions-panel`).classList.add('active');
}

async function installExtensionAction(extensionId) {
  try {
    const available = await window.__TAURI__.invoke('get_available_extensions');
    const ext = available.find(e => e.id === extensionId);
    if (ext) {
      await window.__TAURI__.invoke('install_extension', { metadata: ext });
      showToast(`Extension ${ext.name} installed`, 'success');
      loadExtensions();
    }
  } catch (error) {
    console.error('Failed to install extension:', error);
    showToast('Failed to install extension', 'error');
  }
}

async function uninstallExtensionAction(extensionId) {
  if (confirm('Are you sure you want to uninstall this extension?')) {
    try {
      await window.__TAURI__.invoke('uninstall_extension', { extensionId });
      showToast('Extension uninstalled', 'success');
      loadExtensions();
    } catch (error) {
      console.error('Failed to uninstall extension:', error);
      showToast('Failed to uninstall extension', 'error');
    }
  }
}

async function toggleExtensionStatus(extensionId, currentlyEnabled) {
  try {
    if (currentlyEnabled) {
      await window.__TAURI__.invoke('disable_extension', { extensionId });
    } else {
      await window.__TAURI__.invoke('enable_extension', { extensionId });
    }
    loadExtensions();
  } catch (error) {
    console.error('Failed to toggle extension:', error);
    showToast('Failed to toggle extension', 'error');
  }
}

async function refreshExtensions() {
  loadExtensions();
}

// Routines Management
async function loadRoutines() {
  try {
    const routines = await window.__TAURI__.invoke('get_routines');
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
    
    await window.__TAURI__.invoke('create_routine', {
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
    await window.__TAURI__.invoke('trigger_routine', { routineId });
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
      await window.__TAURI__.invoke('pause_routine', { routineId });
    } else if (currentStatus === 'Paused') {
      await window.__TAURI__.invoke('enable_routine', { routineId });
    } else {
      await window.__TAURI__.invoke('enable_routine', { routineId });
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
      await window.__TAURI__.invoke('delete_routine', { routineId });
      showToast('Routine deleted', 'success');
      loadRoutines();
    } catch (error) {
      console.error('Failed to delete routine:', error);
      showToast('Failed to delete routine', 'error');
    }
  }
}
