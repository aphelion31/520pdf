// Run: node static/ui-check.mjs. Uses the installed Playwright MCP runtime, no new dependencies.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { dirname, resolve } from 'node:path';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_PATH || resolve(dirname(process.execPath), '../lib/node_modules/@playwright/mcp/node_modules/playwright'));
const ids = 'merge split organize compress rotate crop watermark page-numbers forms sign image-to-pdf office-to-pdf html-to-pdf compare pdf-to-markdown unlock protect pdfa scan-to-pdf repair redact summarize translate'.split(' ');
function samplePdf() {
  const objects = ['<< /Type /Catalog /Pages 2 0 R >>', '<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>', '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /Resources << >> /Contents 5 0 R >>', '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /Resources << >> /Contents 5 0 R >>', '<< /Length 0 >>\nstream\n\nendstream'];
  let pdf = '%PDF-1.4\n', offsets = [0];
  for (const [index, object] of objects.entries()) { offsets.push(Buffer.byteLength(pdf)); pdf += `${index + 1} 0 obj\n${object}\nendobj\n`; }
  const xref = Buffer.byteLength(pdf); pdf += `xref\n0 ${offsets.length}\n0000000000 65535 f \n` + offsets.slice(1).map(n => `${String(n).padStart(10, '0')} 00000 n \n`).join('') + `trailer\n<< /Size ${offsets.length} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return Buffer.from(pdf);
}
const fields = { compress: [{ name: 'target_kb', label: 'Target KB', type: 'number', value: '250' }], protect: [{ name: 'password', label: 'PDF password', type: 'password', required: true }], crop: ['x', 'y', 'width', 'height'].map(name => ({ name, type: 'number', value: '20' })), redact: [{ name: 'rectangles', type: 'textarea', value: '[]' }], sign: [{ name: 'text', type: 'text', value: '' }] };
const tools = ids.map((id) => ({ id, name: id === 'merge' ? 'Merge PDF' : id === 'protect' ? 'Protect PDF' : id, description: `Process ${id} documents`, category: id === 'merge' ? 'Organize' : 'Convert', accept: '.pdf', multiple: id === 'merge', fields: fields[id] || [] }));
let failCatalog = false;
let failJob = false;
let submitted = '';
const server = createServer(async (request, response) => {
  if (request.url === '/api/tools') {
    response.writeHead(failCatalog ? 503 : 200, { 'Content-Type': 'application/json' });
    return response.end(JSON.stringify({ tools }));
  }
  if (request.method === 'POST') {
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    submitted = Buffer.concat(chunks).toString();
    response.writeHead(failJob ? 422 : 200, failJob ? { 'Content-Type': 'application/json' } : { 'Content-Type': 'application/pdf', 'Content-Disposition': 'attachment; filename="520pdf-result.pdf"' });
    return response.end(failJob ? JSON.stringify({ error: 'Test processing error' }) : '%PDF-1.4\nresult');
  }
  const name = request.url === '/' ? 'index.html' : request.url?.replace('/assets/', '');
  if (!['index.html', 'app.js', 'app.css', 'basecoat.css', 'workbench.js', 'workbench.css', 'pdfjs/pdf.mjs', 'pdfjs/pdf.worker.mjs'].includes(name)) { response.writeHead(404); return response.end(); }
  response.setHeader('Content-Type', name.endsWith('.css') ? 'text/css' : /\.m?js$/.test(name) ? 'application/javascript' : 'text/html');
  response.end(await readFile(new URL(name, import.meta.url)));
});
await new Promise((done) => server.listen(0, '127.0.0.1', done));
const browser = await chromium.launch({ headless: true, ...(process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {}) });
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const url = `http://127.0.0.1:${server.address().port}`;
  await page.goto(url);
  await page.waitForFunction(() => document.querySelectorAll('.tool-card').length === 23);
  assert.equal(await page.locator('.tool-card').count(), 23);
  await page.getByLabel('Search PDF tools').fill('merge');
  assert.equal(await page.locator('.tool-card').count(), 1);
  await page.getByRole('button', { name: 'Merge PDF' }).click();
  await page.locator('#file-input').setInputFiles([{ name: 'first.pdf', mimeType: 'application/pdf', buffer: Buffer.from('%PDF-first') }, { name: 'second.pdf', mimeType: 'application/pdf', buffer: Buffer.from('%PDF-second') }]);
  await page.getByLabel('Move earlier: second.pdf').click();
  assert.match(await page.locator('.file-name').first().textContent(), /second.pdf/);
  await page.getByRole('button', { name: 'Process', exact: true }).click();
  await page.locator('#download-panel').waitFor({ state: 'visible' });
  assert.ok(submitted.indexOf('second.pdf') < submitted.indexOf('first.pdf'));
  assert.equal(await page.locator('#download').getAttribute('download'), '520pdf-result.pdf');
  await page.getByRole('button', { name: 'Close tool' }).click();
  await page.getByLabel('Search PDF tools').fill('protect');
  await page.getByRole('button', { name: 'Protect PDF' }).click();
  assert.equal(await page.getByLabel('PDF password').getAttribute('type'), 'password');
  await page.locator('#file-input').setInputFiles({ name: 'bad.txt', mimeType: 'text/plain', buffer: Buffer.from('bad') });
  assert.match(await page.locator('#job-error').textContent(), /not a supported file type/);
  await page.locator('#file-input').setInputFiles({ name: 'document.pdf', mimeType: 'application/pdf', buffer: Buffer.from('%PDF') });
  await page.getByLabel('PDF password').fill('example-password');
  failJob = true;
  await page.getByRole('button', { name: 'Process', exact: true }).click();
  await page.getByText('Test processing error', { exact: true }).waitFor();
  assert.equal(await page.locator('#work-controls').isDisabled(), false);
  await page.getByRole('button', { name: 'Close tool' }).click();
  await page.getByLabel('Search PDF tools').fill('');
  await page.getByRole('button', { name: 'Organize', exact: true }).click();
  assert.equal(await page.locator('.tool-card').count(), 1);
  await page.getByRole('button', { name: 'All tools', exact: true }).click();
  await page.locator('.tool-card').filter({ hasText: 'Process organize' }).click();
  await page.locator('#file-input').setInputFiles({ name: 'two-pages.pdf', mimeType: 'application/pdf', buffer: samplePdf() });
  await page.locator('.page-card').nth(1).waitFor();
  await page.getByLabel('Move page 2 earlier').click();
  assert.match(await page.locator('.page-card').first().textContent(), /Page 2/);
  assert.equal(await page.locator('[name=pages]').inputValue(), '2,1');
  await page.getByRole('button', { name: 'Close tool' }).click();
  await page.locator('.tool-card').filter({ hasText: 'Process rotate' }).click();
  await page.locator('#file-input').setInputFiles({ name: 'two-pages.pdf', mimeType: 'application/pdf', buffer: samplePdf() });
  await page.getByLabel('Rotate page 2 90 degrees clockwise').click();
  assert.equal(await page.locator('[name=rotations]').inputValue(), '{"1":0,"2":90}');
  await page.getByRole('button', { name: 'Close tool' }).click();
  await page.locator('.tool-card').filter({ hasText: 'Process crop' }).click();
  await page.locator('#file-input').setInputFiles({ name: 'two-pages.pdf', mimeType: 'application/pdf', buffer: samplePdf() });
  await page.getByLabel('Rectangle page').selectOption('2');
  assert.equal(await page.locator('[name=pages]').inputValue(), '2');
  await page.getByRole('button', { name: 'Close tool' }).click();
  await page.locator('.tool-card').filter({ hasText: 'Process redact' }).click();
  await page.locator('#file-input').setInputFiles({ name: 'two-pages.pdf', mimeType: 'application/pdf', buffer: samplePdf() });
  await page.getByLabel('Rectangle page').selectOption('2');
  for (const [name, value] of Object.entries({ x: '10', y: '20', width: '30', height: '40' })) await page.locator(`[data-rect=${name}]`).fill(value);
  await page.getByRole('button', { name: 'Add redaction area' }).click();
  await page.getByLabel('Rectangle page').selectOption('1');
  await page.getByRole('button', { name: 'Add redaction area' }).click();
  assert.deepEqual(JSON.parse(await page.locator('[name=rectangles]').inputValue()).map(r => r.page), [2, 1]);
  await page.getByLabel('Remove rectangle 1').click();
  assert.deepEqual(JSON.parse(await page.locator('[name=rectangles]').inputValue()).map(r => r.page), [1]);
  await page.getByRole('button', { name: 'Close tool' }).click();
  await page.locator('.tool-card').filter({ hasText: 'Process sign' }).click();
  await page.locator('#file-input').setInputFiles({ name: 'two-pages.pdf', mimeType: 'application/pdf', buffer: samplePdf() });
  await page.locator('.signature-canvas').scrollIntoViewIfNeeded();
  const sig = await page.locator('.signature-canvas').boundingBox();
  await page.mouse.move(sig.x + 20, sig.y + 20); await page.mouse.down(); await page.mouse.move(sig.x + 80, sig.y + 50); await page.mouse.up();
  assert.equal(await page.locator('.placement-image').isVisible(), true);
  await page.getByLabel('Signature left (points)').fill('60');
  assert.equal(await page.locator('.placement-image').evaluate(e => e.style.left), '20%');
  assert.ok((await page.locator('[name=image_base64]').inputValue()).length > 100);
  await page.getByRole('button', { name: 'Close tool' }).click();
  failJob = false;
  await page.locator('.tool-card').filter({ hasText: 'Process compress' }).click();
  await page.locator('#file-input').setInputFiles({ name: 'sample.pdf', mimeType: 'application/pdf', buffer: samplePdf() });
  await page.getByRole('button', { name: 'Process', exact: true }).click();
  await page.locator('#compression-result').waitFor();
  await page.getByRole('button', { name: 'Compare original and result' }).click();
  assert.equal(await page.locator('#compression-result iframe').count(), 2);
  await page.getByRole('button', { name: 'Close tool' }).click();
  await page.setViewportSize({ width: 375, height: 812 });
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
  await page.getByRole('button', { name: 'Merge PDF' }).click();
  await page.keyboard.press('Escape');
  assert.equal(await page.locator('#workspace').evaluate((dialog) => dialog.open), false);
  failCatalog = true;
  await page.reload();
  await page.locator('#retry').waitFor({ state: 'visible' });
  failCatalog = false;
  await page.getByRole('button', { name: 'Try loading tools again' }).click();
  await page.waitForFunction(() => document.querySelectorAll('.tool-card').length === 23);
  assert.deepEqual(errors, []);
  console.log('PASS: 23 tools, search/category, upload validation/order, options, download, server errors/retry, mobile overflow, Escape, no browser errors');
} finally {
  await browser.close();
  server.close();
}
