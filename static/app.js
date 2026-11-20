'use strict';

const $ = (id) => document.getElementById(id);
const state = { tools: [], category: 'All tools', selected: null, files: [], busy: false, url: null };
const icons = {
  merge: 'M8 3v6l4 4 4-4V3M12 13v8m-4-4 4 4 4-4',
  split: 'M12 21v-8M5 3v5l7 5 7-5V3M2 6l3-3 3 3m8 0 3-3 3 3',
  compress: 'M3 8h5V3m13 5h-5V3M3 16h5v5m13-5h-5v5M8 8 3 3m13 5 5-5M8 16l-5 5m13-5 5 5',
  rotate: 'M20 10a8 8 0 1 0-2 8M20 4v6h-6',
  crop: 'M7 2v15h15M2 7h15v15',
  protect: 'M12 2 4 5v6c0 5 8 10 8 10s8-5 8-10V5l-8-3m-4 10 3 3 5-6',
  unlock: 'M8 11V7a4 4 0 0 1 8 0M5 11h14v10H5V11m7 4v2',
  sign: 'm4 16 12-12 4 4L8 20H4v-4m10-10 4 4M12 21h9',
  compare: 'M4 3h6v18H4V3m10 0h6v18h-6V3M7 7h0m10 10h0',
  default: 'M6 2h8l4 4v16H6V2m8 0v5h4M9 12h6m-6 4h6'
};
function element(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}
function icon(id) {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('aria-hidden', 'true');
  const path = document.createElementNS(svg.namespaceURI, 'path');
  path.setAttribute('d', icons[id] || icons.default);
  svg.append(path);
  return svg;
}
function showError(message) {
  $('job-error').textContent = message;
  $('job-error').hidden = !message;
}
function clearResult() {
  if (state.url) URL.revokeObjectURL(state.url);
  state.url = null;
  $('download-panel').hidden = true;
  $('download').removeAttribute('href');
  $('job-status').textContent = '';
}
function renderCatalog() {
  const query = $('search').value.trim().toLowerCase();
  const tools = state.tools.filter((tool) =>
    (state.category === 'All tools' || tool.category === state.category) &&
    `${tool.name} ${tool.description} ${tool.id}`.toLowerCase().includes(query));
  $('tool-grid').replaceChildren(...tools.map((tool) => {
    const button = element('button', 'tool-card');
    button.type = 'button';
    button.append(icon(tool.id), element('span', 'tool-name', tool.name), element('span', 'tool-description', tool.description));
    button.addEventListener('click', () => openTool(tool));
    return button;
  }));
  $('catalog-status').textContent = tools.length ? `${tools.length} ${tools.length === 1 ? 'tool' : 'tools'}${query ? ` matching “${$('search').value.trim()}”` : ' for your documents'}` : 'No matching tools. Try another search or category.';
  for (const button of $('categories').children) button.setAttribute('aria-pressed', String(button.textContent === state.category));
}
async function loadCatalog() {
  $('retry').hidden = true;
  $('catalog-status').textContent = 'Loading your PDF tools…';
  try {
    const response = await fetch('/api/tools', { headers: { Accept: 'application/json' } });
    if (!response.ok) throw new Error('The tool catalog is unavailable. Please try again.');
    const catalog = await response.json();
    if (!Array.isArray(catalog.tools) || !catalog.tools.length || !catalog.tools.every((tool) => typeof tool.id === 'string' && typeof tool.name === 'string' && Array.isArray(tool.fields))) throw new Error('The server returned an invalid tool catalog. Please try again.');
    state.tools = catalog.tools;
    state.category = 'All tools';
    $('categories').replaceChildren(...['All tools', ...new Set(state.tools.map((tool) => tool.category).filter(Boolean))].map((category) => {
      const button = element('button', '', category);
      button.type = 'button';
      button.addEventListener('click', () => { state.category = category; renderCatalog(); });
      return button;
    }));
    renderCatalog();
  } catch (error) {
    $('catalog-status').textContent = error.message || 'Unable to connect to the server. Please try again.';
    $('retry').hidden = false;
  }
}
function renderOptions(tool) {
  $('option-fields').replaceChildren(...tool.fields.map((field, index) => {
    const type = field.type || 'text';
    const wrapper = element('div', `option-field${['textarea', 'checkbox'].includes(type) ? ' wide' : ''}${type === 'checkbox' ? ' checkbox' : ''}`);
    const label = element('label', '', field.label || field.name);
    const input = element(type === 'textarea' ? 'textarea' : type === 'select' ? 'select' : 'input');
    input.id = `option-${index}`;
    input.name = field.name;
    label.htmlFor = input.id;
    if (input.tagName === 'INPUT') input.type = ['number', 'password', 'checkbox', 'file', 'email', 'url', 'range'].includes(type) ? type : 'text';
    if (type === 'select') {
      for (const option of field.options || []) {
        const node = element('option', '', typeof option === 'object' ? option.label : option);
        node.value = typeof option === 'object' ? option.value : option;
        input.append(node);
      }
    }
    if (type === 'checkbox') input.checked = field.value === true || field.value === 'true';
    else if (type !== 'file') input.value = field.value ?? '';
    for (const attribute of ['placeholder', 'min', 'max', 'step', 'accept', 'pattern']) if (field[attribute] !== undefined) input.setAttribute(attribute, field[attribute]);
    input.required = Boolean(field.required) || field.name === 'consent';
    if (type === 'password') input.autocomplete = 'off';
    if (type === 'checkbox') wrapper.append(input, label);
    else wrapper.append(label, input);
    if (field.description || field.help) {
      const help = element('small', '', field.description || field.help);
      help.id = `option-help-${index}`;
      input.setAttribute('aria-describedby', help.id);
      wrapper.append(help);
    }
    return wrapper;
  }));
}
function openTool(tool) {
  state.selected = tool;
  state.files = [];
  clearResult();
  showError('');
  $('tool-title').textContent = tool.name;
  $('tool-description').textContent = tool.description;
  $('file-input').value = '';
  $('file-input').accept = Array.isArray(tool.accept) ? tool.accept.join(',') : tool.accept || '';
  $('file-input').multiple = Boolean(tool.multiple);
  $('file-label').textContent = tool.multiple ? 'Choose files' : 'Choose file';
  $('upload-hint').textContent = `Or drop ${tool.multiple ? 'files' : 'a file'} here${tool.accept ? ` (${Array.isArray(tool.accept) ? tool.accept.join(', ') : tool.accept})` : ''}`;
  renderOptions(tool);
  renderFiles();
  window.dispatchEvent(new CustomEvent('workbench:open', { detail: tool }));
  $('workspace').showModal();
}
function fileAccepted(file) {
  const accept = Array.isArray(state.selected.accept) ? state.selected.accept : (state.selected.accept || '').split(',');
  return !accept.length || accept.every((part) => !part.trim()) || accept.some((part) => {
    const rule = part.trim().toLowerCase();
    return rule.startsWith('.') ? file.name.toLowerCase().endsWith(rule) : rule.endsWith('/*') ? file.type.toLowerCase().startsWith(rule.slice(0, -1)) : file.type.toLowerCase() === rule;
  });
}
function addFiles(files) {
  if (state.busy) return;
  const incoming = [...files];
  if (!incoming.length) return;
  if (!state.selected.multiple && incoming.length > 1) { showError('This tool accepts one file at a time. Choose a single file.'); return; }
  const invalid = incoming.find((file) => !fileAccepted(file));
  if (invalid) { showError(`“${invalid.name}” is not a supported file type for this tool.`); return; }
  if (incoming.some((file) => file.size === 0)) { showError('Empty files cannot be processed. Choose a file with content.'); return; }
  const nextFiles = state.selected.multiple ? [...state.files, ...incoming] : incoming;
  if (nextFiles.length > 30) { showError('Choose at most 30 files per job.'); return; }
  if (nextFiles.reduce((sum, file) => sum + file.size, 0) > 49 * 1024 * 1024) { showError('Keep the combined upload under 49 MB to leave room for processing options.'); return; }
  showError('');
  clearResult();
  state.files = nextFiles;
  renderFiles();
  window.dispatchEvent(new CustomEvent('workbench:files', { detail: state.files }));
}
function renderFiles() {
  $('file-list').replaceChildren(...state.files.map((file, index) => {
    const row = element('li');
    const details = element('div', 'file-details');
    details.append(element('span', 'file-name', `${index + 1}. ${file.name}`), element('span', 'file-size', formatSize(file.size)));
    const actions = element('div', 'file-actions');
    const addAction = (text, label, disabled, handler) => {
      const button = element('button', 'btn-ghost', text);
      button.type = 'button';
      button.setAttribute('aria-label', `${label} ${file.name}`);
      button.disabled = disabled;
      button.addEventListener('click', () => {
        handler(); clearResult(); renderFiles();
        window.dispatchEvent(new CustomEvent('workbench:files', { detail: state.files }));
        const nextRow = $('file-list').children[Math.min(index, state.files.length - 1)];
        nextRow?.querySelector('button:not(:disabled)')?.focus();
      });
      actions.append(button);
    };
    if (state.selected.multiple) {
      addAction('↑', 'Move earlier:', index === 0, () => { [state.files[index - 1], state.files[index]] = [state.files[index], state.files[index - 1]]; });
      addAction('↓', 'Move later:', index === state.files.length - 1, () => { [state.files[index + 1], state.files[index]] = [state.files[index], state.files[index + 1]]; });
    }
    addAction('×', 'Remove', false, () => { state.files.splice(index, 1); });
    row.append(details, actions);
    return row;
  }));
  $('file-summary').textContent = state.files.length ? `${state.files.length} ${state.files.length === 1 ? 'file' : 'files'} selected, ${formatSize(state.files.reduce((sum, file) => sum + file.size, 0))}${state.files.length > 1 ? '. Files are processed in this order.' : ''}` : 'No files selected';
  $('process').disabled = !state.files.length;
}
function formatSize(bytes) {
  return bytes < 1024 ? `${bytes} B` : bytes < 1048576 ? `${(bytes / 1024).toFixed(1)} KB` : `${(bytes / 1048576).toFixed(1)} MB`;
}
function downloadName(header) {
  const encoded = /filename\*=UTF-8''([^;]+)/i.exec(header || '');
  if (encoded) { try { return decodeURIComponent(encoded[1]).split(/[\\/]/).pop(); } catch { /* Fall back to the plain filename. */ } }
  const plain = /filename="([^"]+)"|filename=([^;]+)/i.exec(header || '');
  return plain ? (plain[1] || plain[2]).trim().split(/[\\/]/).pop() : `520pdf-${state.selected.id}-result`;
}
async function processFiles(event) {
  event.preventDefault();
  if (state.busy || !state.files.length || !$('tool-form').reportValidity()) return;
  showError('');
  clearResult();
  window.dispatchEvent(new Event('workbench:submit'));
  const data = new FormData();
  for (const file of state.files) data.append('files', file, file.name);
  for (const input of $('option-fields').querySelectorAll('input, select, textarea')) {
    if (input.type === 'file') { for (const file of input.files) data.append(input.name, file, file.name); }
    else data.append(input.name, input.type === 'checkbox' ? String(input.checked) : input.value);
  }
  state.busy = true;
  $('work-controls').disabled = true;
  $('close-workspace').disabled = true;
  $('tool-form').setAttribute('aria-busy', 'true');
  $('job-status').textContent = 'Uploading and processing your documents. Keep this workspace open.';
  $('process').textContent = 'Processing…';
  try {
    const response = await fetch(`/api/tools/${encodeURIComponent(state.selected.id)}`, { method: 'POST', body: data });
    if (!response.ok) {
      let message = `Processing failed (${response.status}). Please try again.`;
      try { const error = await response.json(); if (typeof error.error === 'string') message = error.error; } catch { /* Keep the safe HTTP error. */ }
      throw new Error(message);
    }
    const blob = await response.blob();
    if (!blob.size) throw new Error('The server returned an empty result. Please try again.');
    state.url = URL.createObjectURL(blob);
    $('download').href = state.url;
    $('download').download = downloadName(response.headers.get('Content-Disposition'));
    $('download-panel').hidden = false;
    $('job-status').textContent = 'Processing complete. Your original files have not been changed.';
    $('download').focus();
  } catch (error) {
    $('job-status').textContent = '';
    showError(error.message || 'The connection was interrupted. Please try again.');
  } finally {
    state.busy = false;
    $('work-controls').disabled = false;
    $('close-workspace').disabled = false;
    $('tool-form').setAttribute('aria-busy', 'false');
    $('process').textContent = 'Process';
  }
}
$('search').addEventListener('input', renderCatalog);
$('retry').addEventListener('click', loadCatalog);
$('close-workspace').addEventListener('click', () => { if (!state.busy) $('workspace').close(); });
$('workspace').addEventListener('cancel', (event) => { if (state.busy) event.preventDefault(); });
$('workspace').addEventListener('close', () => { window.dispatchEvent(new Event('workbench:close')); clearResult(); state.files = []; $('file-input').value = ''; $('option-fields').replaceChildren(); });
$('file-input').addEventListener('change', (event) => { addFiles(event.target.files); event.target.value = ''; });
$('tool-form').addEventListener('submit', processFiles);
for (const name of ['dragenter', 'dragover']) $('dropzone').addEventListener(name, (event) => { event.preventDefault(); if (!state.busy) $('dropzone').classList.add('dragging'); });
for (const name of ['dragleave', 'drop']) $('dropzone').addEventListener(name, (event) => { event.preventDefault(); $('dropzone').classList.remove('dragging'); });
$('dropzone').addEventListener('drop', (event) => addFiles(event.dataTransfer.files));
window.addEventListener('beforeunload', (event) => { if (state.busy) { event.preventDefault(); event.returnValue = ''; } });
loadCatalog();
