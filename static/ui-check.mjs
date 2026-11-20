// Run: node static/ui-check.mjs. Uses the installed Playwright MCP runtime, no new dependencies.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { dirname, resolve } from 'node:path';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_PATH || resolve(dirname(process.execPath), '../lib/node_modules/@playwright/mcp/node_modules/playwright'));
const ids = 'merge split organize compress rotate crop watermark page-numbers forms sign image-to-pdf office-to-pdf html-to-pdf compare pdf-to-markdown unlock protect pdfa scan-to-pdf repair redact summarize translate'.split(' ');
const tools = ids.map((id) => ({ id, name: id === 'merge' ? 'Merge PDF' : id === 'protect' ? 'Protect PDF' : id, description: `Process ${id} documents`, category: id === 'merge' ? 'Organize' : 'Convert', accept: '.pdf', multiple: id === 'merge', fields: id === 'protect' ? [{ name: 'password', label: 'PDF password', type: 'password', required: true }] : [] }));
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
  if (!['index.html', 'app.js', 'app.css', 'basecoat.css'].includes(name)) { response.writeHead(404); return response.end(); }
  response.setHeader('Content-Type', name.endsWith('.css') ? 'text/css' : name.endsWith('.js') ? 'application/javascript' : 'text/html');
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
