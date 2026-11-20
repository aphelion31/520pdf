'use strict';

// Local PDF.js only. PDF JavaScript and eval are never enabled for document previews.
(() => {
  const host = document.getElementById('page-workbench');
  const options = document.getElementById('option-fields');
  let tool, documentTask, version = 0, pages = [], dragIndex = null;
  const node = (tag, cls, text) => { const e = document.createElement(tag); if (cls) e.className = cls; if (text !== undefined) e.textContent = text; return e; };
  const field = (name) => options.querySelector(`[name="${name}"]`);
  function ensureField(name, value = '') {
    let input = field(name);
    if (!input) { input = node('input'); input.type = 'hidden'; input.name = name; input.value = value; options.append(input); }
    return input;
  }
  function updateSelection() {
    if (tool.id === 'organize' || tool.id === 'split') {
      ensureField('pages').value = pages.filter(p => p.selected).map(p => p.number).join(',');
      document.getElementById('process').disabled = !pages.some(p => p.selected);
    }
    if (tool.id === 'rotate') ensureField('rotations').value = JSON.stringify(Object.fromEntries(pages.map(p => [p.number, p.rotation])));
  }
  function button(label, action, disabled = false) {
    const b = node('button', 'btn-outline page-action', label); b.type = 'button'; b.disabled = disabled; b.addEventListener('click', action); return b;
  }
  function drawPages() {
    const grid = host.querySelector('.page-grid');
    if (!grid) return;
    grid.replaceChildren(...pages.map((page, index) => {
      const card = node('li', 'page-card');
      card.draggable = tool.id === 'organize';
      if (card.draggable) {
        card.addEventListener('dragstart', e => { dragIndex = index; e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', String(index)); });
        card.addEventListener('dragover', e => e.preventDefault());
        card.addEventListener('drop', e => { e.preventDefault(); if (dragIndex === null || dragIndex === index) return; pages.splice(index, 0, pages.splice(dragIndex, 1)[0]); dragIndex = null; updateSelection(); drawPages(); });
        card.addEventListener('dragend', () => { dragIndex = null; });
      }
      const image = node('div', 'page-image'); image.append(page.canvas);
      page.canvas.style.transform = `rotate(${page.rotation}deg)`;
      card.append(image, node('strong', '', `Page ${page.number}`));
      const actions = node('div', 'page-actions');
      if (tool.id === 'organize' || tool.id === 'split') {
        const label = node('label', 'page-select'); const input = node('input'); input.type = 'checkbox'; input.checked = page.selected;
        input.setAttribute('aria-label', `Include page ${page.number}`);
        input.addEventListener('change', () => { page.selected = input.checked; updateSelection(); }); label.append(input, document.createTextNode(' Include')); actions.append(label);
      }
      if (tool.id === 'organize') {
        const move = (delta) => { const next = index + delta; [pages[index], pages[next]] = [pages[next], pages[index]]; updateSelection(); drawPages(); host.querySelectorAll('.page-card')[next]?.querySelector(`[data-move="${delta}"]`)?.focus(); };
        const up = button('Earlier', () => move(-1), index === 0); up.dataset.move = '-1'; up.setAttribute('aria-label', `Move page ${page.number} earlier`);
        const down = button('Later', () => move(1), index === pages.length - 1); down.dataset.move = '1'; down.setAttribute('aria-label', `Move page ${page.number} later`); actions.append(up, down);
      }
      if (tool.id === 'rotate') { const rotate = button('Rotate 90°', () => { page.rotation = (page.rotation + 90) % 360; updateSelection(); drawPages(); }); rotate.setAttribute('aria-label', `Rotate page ${page.number} 90 degrees clockwise`); actions.append(rotate); }
      card.append(actions); return card;
    }));
  }
  async function showFiles(files) {
    const current = ++version;
    if (documentTask) { documentTask.destroy(); documentTask = null; }
    pages = []; host.replaceChildren(); host.hidden = true;
    const file = files[0];
    if (!tool || !file || !(file.type === 'application/pdf' || /\.pdf$/i.test(file.name))) return;
    host.hidden = false;
    const heading = node('h3', '', 'Document preview');
    const status = node('p', 'preview-status', 'Loading pages locally…'); status.setAttribute('role', 'status'); host.append(heading, status);
    try {
      const pdfjs = await import('/assets/pdfjs/pdf.mjs');
      pdfjs.GlobalWorkerOptions.workerSrc = '/assets/pdfjs/pdf.worker.mjs';
      const bytes = await file.arrayBuffer();
      if (current !== version) return;
      documentTask = pdfjs.getDocument({ data: bytes, isEvalSupported: false, enableXfa: false, disableFontFace: true, useWorkerFetch: false });
      const pdf = await documentTask.promise;
      if (current !== version) return;
      if (pdf.numPages > 100 && ['organize', 'split', 'rotate'].includes(tool.id)) { status.textContent = 'Visual page editing supports up to 100 pages. Use the page fields below for this document.'; return; }
      const grid = node('ol', 'page-grid'); grid.setAttribute('aria-label', 'PDF pages'); host.append(grid);
      const count = Math.min(pdf.numPages, 100);
      grid.inert = true;
      status.textContent = `${pdf.numPages} ${pdf.numPages === 1 ? 'page' : 'pages'} in document${pdf.numPages > count ? '. Showing the first 100 pages. Use page fields for the full document.' : ' loaded locally'}${tool.id === 'organize' ? ' Drag pages or use Earlier and Later to reorder.' : ''}`;
      for (let number = 1; number <= count; number++) {
        if (current !== version) return;
        const page = await pdf.getPage(number);
        const viewport = page.getViewport({ scale: 1 });
        const scale = Math.min(180 / viewport.width, 240 / viewport.height, 1);
        const view = page.getViewport({ scale });
        const canvas = node('canvas'); canvas.width = Math.ceil(view.width); canvas.height = Math.ceil(view.height);
        canvas.setAttribute('aria-label', `Preview of page ${number}`);
        await page.render({ canvasContext: canvas.getContext('2d'), viewport: view }).promise;
        pages.push({ number, canvas, rotation: 0, selected: true, width: viewport.width, height: viewport.height, originalRotation: viewport.rotation, view: page.view });
        drawPages();
      }
      grid.inert = false;
      updateSelection();
      if (['crop', 'redact'].includes(tool.id)) addRectangleEditor();
      if (['watermark', 'page-numbers', 'sign'].includes(tool.id)) addPlacementPreview();
      if (tool.id === 'sign') addSignatureEditor();
    } catch (error) {
      if (current === version) status.textContent = `Preview unavailable: ${error.message || 'Unable to read this PDF'}. You can still use the fields below.`;
    }
  }
  function addRectangleEditor() {
    const page = pages[0]; if (!page) return;
    if (page.originalRotation || page.view[0] !== 0 || page.view[1] !== 0) { host.append(node('p', 'preview-status', 'Visual rectangle editing is unavailable for rotated or offset pages. Use the coordinate fields below.')); return; }
    const panel = node('div', 'rectangle-editor');
    panel.append(node('h4', '', tool.id === 'crop' ? 'Crop area on page 1' : 'Redaction area on page 1'), node('p', 'preview-status', 'Drag on the page to set the rectangle. Coordinates are in PDF points from the bottom left.'));
    const stage = node('div', 'rectangle-stage'); const canvas = node('canvas'); canvas.width = page.canvas.width; canvas.height = page.canvas.height; canvas.getContext('2d').drawImage(page.canvas, 0, 0);
    const box = node('div', 'rectangle-box'); stage.append(canvas, box); panel.append(stage);
    const names = ['x', 'y', 'width', 'height'];
    const values = Object.fromEntries(names.map(name => [name, field(name)]));
    if (tool.id === 'crop' && names.some(name => !values[name])) { panel.append(node('p', 'preview-status', 'Crop coordinate fields are unavailable.')); host.append(panel); return; }
    if (tool.id === 'redact') {
      const controls = node('div', 'rectangle-inputs');
      for (const name of names) { const label = node('label', '', name[0].toUpperCase() + name.slice(1)); const input = node('input'); input.type = 'number'; input.min = '0'; input.step = 'any'; input.value = '0'; input.dataset.rect = name; label.append(input); controls.append(label); values[name] = input; }
      panel.append(controls);
    }
    const redraw = () => {
      const x = +values.x.value || 0, y = +values.y.value || 0, w = +values.width.value || 0, h = +values.height.value || 0;
      Object.assign(box.style, { left: `${x / page.width * 100}%`, top: `${(page.height - y - h) / page.height * 100}%`, width: `${w / page.width * 100}%`, height: `${h / page.height * 100}%` });
      if (tool.id === 'redact') ensureField('rectangles').value = JSON.stringify(w > 0 && h > 0 ? [{ page: 1, x, y, width: w, height: h }] : []);
    };
    for (const input of Object.values(values)) input?.addEventListener('input', redraw);
    let start;
    const point = e => { const r = stage.getBoundingClientRect(); return { x: Math.max(0, Math.min(1, (e.clientX-r.left)/r.width)), y: Math.max(0, Math.min(1, (e.clientY-r.top)/r.height)) }; };
    stage.addEventListener('pointerdown', e => { start = point(e); stage.setPointerCapture(e.pointerId); });
    stage.addEventListener('pointermove', e => { if (!start) return; const end = point(e); values.x.value = Math.round(Math.min(start.x,end.x)*page.width); values.y.value = Math.round((1-Math.max(start.y,end.y))*page.height); values.width.value = Math.round(Math.abs(start.x-end.x)*page.width); values.height.value = Math.round(Math.abs(start.y-end.y)*page.height); redraw(); });
    stage.addEventListener('pointerup', () => { start = null; }); stage.addEventListener('pointercancel', () => { start = null; });
    host.append(panel); redraw();
  }
  function addPlacementPreview() {
    const page = pages[0]; if (!page) return;
    const panel = node('div', 'placement-preview'); panel.append(node('h4', '', 'Placement preview on page 1'));
    const stage = node('div', 'placement-stage'); const canvas = node('canvas'); canvas.width = page.canvas.width; canvas.height = page.canvas.height; canvas.getContext('2d').drawImage(page.canvas, 0, 0);
    const overlay = node('span', 'placement-label'); stage.append(canvas, overlay); panel.append(stage); host.append(panel);
    const update = () => { overlay.textContent = field('text')?.value || (tool.id === 'page-numbers' ? (field('format')?.value || '{n}').replace('{n}', field('start')?.value || '1').replace('{total}', String(pages.length)) : 'Preview'); const position = field('position')?.value || 'left'; const x = field('x')?.value ? +field('x').value : position === 'center' ? page.width/2 : position === 'right' ? page.width-36 : 36; const y = field('y')?.value ? +field('y').value : tool.id === 'page-numbers' ? 24 : page.height/2; overlay.style.left = `${Math.max(0,Math.min(100,x/page.width*100))}%`; overlay.style.bottom = `${Math.max(0,Math.min(100,y/page.height*100))}%`; overlay.style.opacity = field('opacity')?.value || '1'; overlay.style.transform = `rotate(${-Number(field('rotation')?.value || 0)}deg)`; };
    options.addEventListener('input', update, { signal: placementAbort.signal }); options.addEventListener('change', update, { signal: placementAbort.signal }); update();
  }
  function addSignatureEditor() {
    const panel = node('div', 'signature-editor'); panel.append(node('h4', '', 'Draw or upload a signature'), node('p', 'preview-status', 'Draw with a pointer, upload an image, or use the signature text field below. This is a visible signature, not a digital certificate.'));
    const canvas = node('canvas', 'signature-canvas'); canvas.width = 360; canvas.height = 120; canvas.setAttribute('aria-label', 'Signature drawing area. Alternatively upload an image or enter signature text.');
    const context = canvas.getContext('2d'); context.strokeStyle = '#242427'; context.lineWidth = 2; context.lineCap = 'round';
    const actions = node('div', 'signature-actions'); const uploadLabel = node('label', '', 'Upload signature image'); const upload = node('input'); upload.type = 'file'; upload.accept = 'image/png,image/jpeg'; uploadLabel.append(upload);
    const save = () => { ensureField('image_base64').value = canvas.toDataURL('image/png').split(',')[1]; const text = field('text'); if (text) text.required = false; };
    const clear = button('Clear signature', () => { context.clearRect(0, 0, canvas.width, canvas.height); field('image_base64')?.remove(); upload.value = ''; upload.setCustomValidity(''); });
    let drawing = false; const point = e => { const r = canvas.getBoundingClientRect(); return [(e.clientX-r.left)*canvas.width/r.width, (e.clientY-r.top)*canvas.height/r.height]; };
    canvas.addEventListener('pointerdown', e => { drawing = true; canvas.setPointerCapture(e.pointerId); context.beginPath(); context.moveTo(...point(e)); });
    canvas.addEventListener('pointermove', e => { if (!drawing) return; context.lineTo(...point(e)); context.stroke(); });
    canvas.addEventListener('pointerup', () => { if (drawing) save(); drawing = false; }); canvas.addEventListener('pointercancel', () => { drawing = false; });
    upload.addEventListener('change', async () => { const file = upload.files[0]; if (!file) return; if (!['image/png', 'image/jpeg'].includes(file.type) || file.size > 5 * 1024 * 1024) { upload.setCustomValidity('Choose a PNG or JPEG under 5 MB.'); upload.reportValidity(); return; } upload.setCustomValidity(''); try { const image = await createImageBitmap(file); context.clearRect(0,0,canvas.width,canvas.height); const scale = Math.min(canvas.width/image.width, canvas.height/image.height); context.drawImage(image, 0, 0, image.width*scale, image.height*scale); image.close(); save(); } catch { upload.setCustomValidity('Unable to read this image.'); upload.reportValidity(); } });
    actions.append(uploadLabel, clear); panel.append(canvas, actions); host.append(panel);
  }
  let placementAbort = new AbortController();
  window.addEventListener('workbench:open', e => { tool = e.detail; placementAbort.abort(); placementAbort = new AbortController(); });
  window.addEventListener('workbench:files', e => showFiles(e.detail));
  window.addEventListener('workbench:close', () => { version++; placementAbort.abort(); if (documentTask) documentTask.destroy(); documentTask = null; host.replaceChildren(); host.hidden = true; });
})();
