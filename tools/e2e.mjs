// End-to-end Chromium verification & retina screenshot capture for 520pdf.
// Run: BASE=http://127.0.0.1:8080 node tools/e2e.mjs
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";

const require = createRequire(import.meta.url);
const playwrightPath =
  process.env.PLAYWRIGHT_PATH ||
  path.resolve(
    path.dirname(process.execPath),
    "../lib/node_modules/@playwright/mcp/node_modules/playwright",
  );
const { chromium } = require(playwrightPath);

const BASE = process.env.BASE || "http://127.0.0.1:8080";
const SHOT_DIR = path.resolve("docs/screenshots");
const TMP_DIR = "/tmp/520pdf-e2e-fixtures";

async function ensureChromiumExecutable() {
  if (process.env.CHROMIUM_PATH) return process.env.CHROMIUM_PATH;
  const home = process.env.HOME || "";
  const candidates = [
    `${home}/Library/Caches/ms-playwright/chromium_headless_shell-1255/chrome-headless-shell-mac-arm64/chrome-headless-shell`,
    `${home}/Library/Caches/ms-playwright/chromium-1255/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing`,
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
  ];
  for (const p of candidates) {
    try {
      await fs.access(p);
      return p;
    } catch {
      // try next
    }
  }
  return undefined;
}

async function makeFixtures() {
  await fs.mkdir(TMP_DIR, { recursive: true });
  // Build real sample PDF via Rust backend resume-builder so PDF.js renders crisp text + layout
  const sampleJson = JSON.stringify({
    template: "modern",
    personalInfo: {
      fullName: "Nadia Kusuma",
      title: "Staff Systems Engineer",
      email: "nadia@520pdf.dev",
      phone: "+62 812 0000 520",
      address: "Jakarta, Indonesia",
      summary:
        "Sample portfolio document used for local 520pdf verification, compression benchmarks, and visual signature placement.",
    },
    experience: [
      {
        company: "520pdf Core",
        position: "Rust Backend Engineer",
        startDate: "2024",
        endDate: "Present",
        description: "Designed single-binary Axum + lopdf + pdf-writer pipelines.",
      },
    ],
    education: [
      {
        institution: "Institute of Technology",
        degree: "B.S.",
        fieldOfStudy: "Computer Science",
        startDate: "2016",
        endDate: "2020",
      },
    ],
    skills: ["Rust", "Axum", "TypeScript", "React Router", "PDF Internals"],
  });
  const form = new FormData();
  form.append("resume_json", sampleJson);
  const pdfRes = await fetch(`${BASE}/api/tools/resume-builder`, { method: "POST", body: form });
  assert.equal(pdfRes.status, 200, "sample PDF creation failed");
  const pdfBuf = Buffer.from(await pdfRes.arrayBuffer());
  const samplePdfPath = path.join(TMP_DIR, "sample-resume.pdf");
  await fs.writeFile(samplePdfPath, pdfBuf);

  // Build real raster PNG via pdf-to-image endpoint
  const imgForm = new FormData();
  imgForm.append("files", new Blob([pdfBuf], { type: "application/pdf" }), "sample-resume.pdf");
  imgForm.append("dpi", "144");
  imgForm.append("format", "png");
  const zipRes = await fetch(`${BASE}/api/tools/pdf-to-image`, { method: "POST", body: imgForm });
  assert.equal(zipRes.status, 200, "sample PNG creation failed");
  const zipBuf = Buffer.from(await zipRes.arrayBuffer());
  const { execFileSync } = await import("node:child_process");
  const zipPath = path.join(TMP_DIR, "pages.zip");
  const samplePngPath = path.join(TMP_DIR, "sample-page.png");
  await fs.writeFile(zipPath, zipBuf);
  execFileSync("python3", [
    "-c",
    `import zipfile; z=zipfile.ZipFile('${zipPath}'); open('${samplePngPath}','wb').write(z.read(z.namelist()[0]))`,
  ]);

  return { samplePdfPath, samplePngPath };
}

await fs.mkdir(SHOT_DIR, { recursive: true });
const { samplePdfPath, samplePngPath } = await makeFixtures();
const executablePath = await ensureChromiumExecutable();
const browser = await chromium.launch({
  headless: true,
  ...(executablePath ? { executablePath } : {}),
});

const consoleErrors = [];
const pageErrors = [];
const failedRequests = [];

try {
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: 2,
  });
  const page = await context.newPage();
  page.on("console", (msg) => {
    if (msg.type() === "error") consoleErrors.push(msg.text());
  });
  page.on("pageerror", (err) => pageErrors.push(err.message));
  page.on("response", (res) => {
    if (res.status() >= 400) failedRequests.push(`${res.status()} ${res.url()}`);
  });

  // 1. Home + DropHero with vault carry-over + tools grid icons
  await page.goto(`${BASE}/`, { waitUntil: "networkidle" });
  await page.locator('input[type="file"]').first().setInputFiles([samplePdfPath, samplePngPath]);
  await page.getByText("2 files in File Vault").waitFor();
  await page.waitForTimeout(300);
  await page.screenshot({ path: path.join(SHOT_DIR, "home.png"), fullPage: false });
  console.log("Captured home.png");

  await page.locator("#tools").scrollIntoViewIfNeeded();
  await page.waitForTimeout(250);
  await page.screenshot({ path: path.join(SHOT_DIR, "tools-grid.png"), fullPage: false });
  console.log("Captured tools-grid.png");

  // 1b. Login page (Argon2id workspace sign-in)
  await page.goto(`${BASE}/login`, { waitUntil: "networkidle" });
  await page.waitForTimeout(250);
  await page.screenshot({ path: path.join(SHOT_DIR, "login.png"), fullPage: false });
  console.log("Captured login.png");

  // 2. Compress PDF (carries sample-resume.pdf automatically from File Vault)
  await page.goto(`${BASE}/compress-pdf`, { waitUntil: "networkidle" });
  await page.locator('input[type="file"]').setInputFiles(samplePdfPath);
  await page.getByRole("button", { name: "Compress PDF" }).click();
  await page.getByText(/Ready:/).waitFor({ timeout: 15000 });
  await page.getByRole("button", { name: "Compare original vs compressed" }).click();
  await page.getByRole("button", { name: "Hide side-by-side comparison" }).scrollIntoViewIfNeeded();
  await page.waitForTimeout(500);
  await page.screenshot({ path: path.join(SHOT_DIR, "compress-pdf.png"), fullPage: false });
  console.log("Captured compress-pdf.png");

  // 3. Compress Image
  await page.goto(`${BASE}/compress-image`, { waitUntil: "networkidle" });
  await page.locator('input[type="file"]').setInputFiles(samplePngPath);
  await page.getByRole("button", { name: "Compress Image" }).click();
  await page.getByText(/Ready:/).waitFor({ timeout: 15000 });
  await page.waitForTimeout(300);
  await page.screenshot({ path: path.join(SHOT_DIR, "compress-image.png"), fullPage: false });
  console.log("Captured compress-image.png");

  // 4. Sign PDF (draw stroke on pad + live page preview + sign)
  await page.goto(`${BASE}/sign-pdf`, { waitUntil: "networkidle" });
  await page.locator('input[type="file"]').first().setInputFiles(samplePdfPath);
  await page.waitForTimeout(500);
  const pad = page.locator("canvas").first();
  const box = await pad.boundingBox();
  assert.ok(box, "signature canvas missing");
  await page.mouse.move(box.x + 40, box.y + 70);
  await page.mouse.down();
  await page.mouse.move(box.x + 120, box.y + 35);
  await page.mouse.move(box.x + 190, box.y + 80);
  await page.mouse.move(box.x + 280, box.y + 50);
  await page.mouse.up();
  await page.getByRole("button", { name: "Sign & Download PDF" }).click();
  await page.getByText(/Ready:/).waitFor({ timeout: 15000 });
  await page.waitForTimeout(300);
  await page.screenshot({ path: path.join(SHOT_DIR, "sign-pdf.png"), fullPage: false });
  console.log("Captured sign-pdf.png");

  // 5. Equation to Image (live KaTeX + PNG render)
  await page.goto(`${BASE}/equation-to-image`, { waitUntil: "networkidle" });
  await page.getByRole("button", { name: "Schrödinger" }).click();
  await page.getByRole("button", { name: /Render PNG Image/ }).click();
  await page.getByText(/Exported \d+ × \d+ px/).waitFor({ timeout: 10000 });
  await page.waitForTimeout(300);
  await page.screenshot({ path: path.join(SHOT_DIR, "equation-to-image.png"), fullPage: false });
  console.log("Captured equation-to-image.png");

  // 6. Resume Builder (live A4 preview + Rust PDF export)
  await page.goto(`${BASE}/resume-builder`, { waitUntil: "networkidle" });
  await page.getByRole("button", { name: "Export Resume PDF (Rust)" }).click();
  await page.getByText(/Ready:/).waitFor({ timeout: 15000 });
  await page.waitForTimeout(300);
  await page.screenshot({ path: path.join(SHOT_DIR, "resume-builder.png"), fullPage: false });
  console.log("Captured resume-builder.png");

  // 7. LaTeX Resume (template + live preview + Rust PDF export)
  await page.goto(`${BASE}/latex-resume`, { waitUntil: "networkidle" });
  await page.getByRole("button", { name: "Export A4 PDF" }).click();
  await page.getByText(/Ready:/).waitFor({ timeout: 15000 });
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.waitForTimeout(300);
  await page.screenshot({ path: path.join(SHOT_DIR, "latex-resume.png"), fullPage: false });
  console.log("Captured latex-resume.png");

  // 8. Mobile viewport check (no horizontal overflow)
  await page.setViewportSize({ width: 375, height: 812 });
  await page.goto(`${BASE}/`, { waitUntil: "networkidle" });
  const fits = await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth);
  assert.equal(fits, true, "horizontal overflow on mobile viewport");

  assert.deepEqual(consoleErrors, [], `console errors: ${consoleErrors.join("; ")}`);
  assert.deepEqual(pageErrors, [], `page errors: ${pageErrors.join("; ")}`);
  assert.deepEqual(failedRequests, [], `failed requests: ${failedRequests.join("; ")}`);
  console.log("PASS: all 9 retina screenshots + zero console/page/network errors + mobile responsive");
} finally {
  await browser.close();
}
