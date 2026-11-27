// Record a short interactive walkthrough of 520pdf -> docs/media/demo.mp4 + demo.webp.
// Uses Chromium CDP Page.startScreencast + system ffmpeg/img2webp (no Playwright ffmpeg needed).
// Run: BASE=http://127.0.0.1:8080 node tools/record-demo.mjs
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
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
const MEDIA_DIR = path.resolve("docs/media");
const TMP_DIR = "/tmp/520pdf-demo-record";

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

async function prepareFixtures() {
  await fs.rm(TMP_DIR, { recursive: true, force: true });
  await fs.mkdir(path.join(TMP_DIR, "raw"), { recursive: true });
  await fs.mkdir(path.join(TMP_DIR, "webp"), { recursive: true });
  await fs.mkdir(MEDIA_DIR, { recursive: true });

  const sampleJson = JSON.stringify({
    template: "modern",
    personalInfo: {
      fullName: "Nadia Kusuma",
      title: "Staff Systems Engineer",
      email: "nadia@520pdf.dev",
      phone: "+62 812 0000 520",
      address: "Jakarta, Indonesia",
      summary: "Designed single-binary Rust Axum + lopdf + pdf-writer pipelines.",
    },
    experience: [
      {
        company: "520pdf Core",
        position: "Rust Backend Engineer",
        startDate: "2024",
        endDate: "Present",
        description: "Built local PDF, Office, and resume workflows.",
      },
    ],
    education: [
      {
        institution: "EEPIS",
        degree: "B.Eng. Informatics",
        startDate: "2020",
        endDate: "2024",
      },
    ],
    skills: ["Rust", "Axum", "React Router", "TypeScript"],
  });
  const form = new FormData();
  form.append("resume_json", sampleJson);
  const res = await fetch(`${BASE}/api/tools/resume-builder`, { method: "POST", body: form });
  assert.equal(res.status, 200, `resume-builder fixture failed (${res.status})`);
  const pdfPath = path.join(TMP_DIR, "sample-resume.pdf");
  await fs.writeFile(pdfPath, Buffer.from(await res.arrayBuffer()));

  const username = `demo_${Date.now()}`;
  const password = "DemoPassword520!";
  const regRes = await fetch(`${BASE}/api/auth/register`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ username, password }),
  });
  assert.equal(regRes.status, 201, `demo auth register failed (${regRes.status})`);
  const setCookie = regRes.headers.get("set-cookie") || "";
  const m = /520pdf_session=([^;]+)/.exec(setCookie);
  assert.ok(m, "missing 520pdf_session cookie");

  return { pdfPath, password, sessionCookie: m[1] };
}

const { pdfPath, password, sessionCookie } = await prepareFixtures();
const executablePath = await ensureChromiumExecutable();
const browser = await chromium.launch({ headless: true, executablePath });
const rawDir = path.join(TMP_DIR, "raw");
const frames = [];
let endTimeMs = 0;

try {
  const baseUrl = new URL(BASE);
  const context = await browser.newContext({
    viewport: { width: 1280, height: 720 },
    deviceScaleFactor: 1,
  });
  await context.addCookies([
    {
      name: "520pdf_session",
      value: sessionCookie,
      domain: baseUrl.hostname,
      path: "/",
      httpOnly: true,
      sameSite: "Lax",
    },
  ]);

  const page = await context.newPage();
  const cdp = await context.newCDPSession(page);
  let frameSeq = 0;
  let writeChain = Promise.resolve();

  cdp.on("Page.screencastFrame", ({ data, metadata, sessionId }) => {
    const idx = frameSeq++;
    const ts = metadata.timestamp ? metadata.timestamp * 1000 : Date.now();
    const outPath = path.join(rawDir, `f_${String(idx).padStart(5, "0")}.png`);
    writeChain = writeChain.then(() => fs.writeFile(outPath, Buffer.from(data, "base64")));
    frames.push({ path: outPath, ts });
    cdp.send("Page.screencastFrameAck", { sessionId }).catch(() => {});
  });

  // Pre-warm route before starting recording
  await page.goto(`${BASE}/`, { waitUntil: "networkidle" });
  await cdp.send("Page.startScreencast", {
    format: "png",
    maxWidth: 1280,
    maxHeight: 720,
    everyNthFrame: 1,
  });

  // Scene 1: Home DropHero -> File Vault -> SlidingTabs filter
  await page.waitForTimeout(600);
  await page.locator('input[type="file"]').first().setInputFiles(pdfPath);
  await page.getByText("1 file in File Vault").waitFor();
  await page.waitForTimeout(750);
  await page.locator("#tools").scrollIntoViewIfNeeded();
  await page.waitForTimeout(450);
  await page.getByRole("button", { name: "Optimize" }).click();
  await page.waitForTimeout(550);
  await page.getByRole("button", { name: "Create" }).click();
  await page.waitForTimeout(550);
  await page.getByRole("button", { name: "All" }).click();
  await page.waitForTimeout(450);

  // Scene 2: Compress PDF + live progress -> Ready banner
  await page.goto(`${BASE}/compress-pdf`, { waitUntil: "networkidle" });
  await page.locator('input[type="file"]').setInputFiles(pdfPath);
  await page.waitForTimeout(450);
  await page.getByRole("button", { name: /Balanced/ }).click();
  await page.waitForTimeout(350);
  await page.getByRole("button", { name: /Maximum shrink/ }).click();
  await page.waitForTimeout(350);
  await page.getByRole("button", { name: "Compress PDF" }).click();
  await page.getByText(/Ready:/).waitFor({ timeout: 15000 });
  await page.waitForTimeout(900);

  // Scene 3: Resume Builder -> step tabs -> Export Resume PDF
  await page.goto(`${BASE}/resume-builder`, { waitUntil: "networkidle" });
  await page.waitForTimeout(700);
  await page.getByRole("button", { name: /2\. Experience/ }).click();
  await page.waitForTimeout(650);
  await page.getByRole("button", { name: /3\. Education/ }).click();
  await page.waitForTimeout(650);
  await page.getByRole("button", { name: /1\. Personal/ }).click();
  await page.waitForTimeout(450);
  await page.getByRole("button", { name: "Export Resume PDF" }).click();
  await page.getByText(/Ready:/).waitFor({ timeout: 15000 });
  await page.waitForTimeout(900);

  // Scene 4: API Keys -> Create -> Password-gated Reveal -> CORS panel
  await page.goto(`${BASE}/api-keys`, { waitUntil: "networkidle" });
  await page.waitForTimeout(500);
  await page.getByPlaceholder(/CI Pipeline/).fill("Production CI");
  await page.getByRole("button", { name: "Generate API Key" }).click();
  await page.getByTestId("minted-token-banner").waitFor({ timeout: 10000 });
  await page.waitForTimeout(700);
  await page.getByRole("button", { name: "View Production CI" }).click();
  await page.getByRole("dialog").waitFor({ timeout: 5000 });
  await page.waitForTimeout(550);
  await page.locator('input[name="reveal_password"]').fill(password);
  await page.getByRole("button", { name: "Reveal Token" }).click();
  await page.locator('[data-testid^="revealed-token-"]').waitFor({ timeout: 5000 });
  await page.waitForTimeout(800);
  await page.getByText("API CORS Origins").scrollIntoViewIfNeeded();
  await page.waitForTimeout(850);

  endTimeMs = frames.length ? frames[frames.length - 1].ts + 450 : 0;
  await cdp.send("Page.stopScreencast");
  await writeChain;
  await context.close();
} finally {
  await browser.close();
}

assert.ok(frames.length >= 10, `too few screencast frames (${frames.length})`);

const concatLines = ["ffconcat version 1.0"];
for (let i = 0; i < frames.length; i++) {
  const cur = frames[i];
  const nextTs = i + 1 < frames.length ? frames[i + 1].ts : endTimeMs;
  const durSec = Math.max(0.033, Math.min(2.5, (nextTs - cur.ts) / 1000));
  concatLines.push(`file '${cur.path}'`);
  concatLines.push(`duration ${durSec.toFixed(4)}`);
}
concatLines.push(`file '${frames[frames.length - 1].path}'`);
const concatPath = path.join(TMP_DIR, "frames.ffconcat");
await fs.writeFile(concatPath, concatLines.join("\n"));

const mp4Path = path.join(MEDIA_DIR, "demo.mp4");
const webpPath = path.join(MEDIA_DIR, "demo.webp");
const webpFramesDir = path.join(TMP_DIR, "webp");

execFileSync(
  "ffmpeg",
  [
    "-y",
    "-f",
    "concat",
    "-safe",
    "0",
    "-i",
    concatPath,
    "-vf",
    "fps=24,format=yuv420p",
    "-c:v",
    "libx264",
    "-movflags",
    "+faststart",
    "-crf",
    "24",
    mp4Path,
  ],
  { stdio: "inherit" },
);

execFileSync(
  "ffmpeg",
  ["-y", "-i", mp4Path, "-vf", "fps=10,scale=960:-1:flags=lanczos", `${webpFramesDir}/f_%04d.png`],
  { stdio: "inherit" },
);

const webpFrames = (await fs.readdir(webpFramesDir))
  .filter((f) => f.endsWith(".png"))
  .sort()
  .map((f) => path.join(webpFramesDir, f));
assert.ok(webpFrames.length > 0, "no webp frames extracted");

execFileSync(
  "img2webp",
  ["-loop", "0", "-d", "100", "-lossy", "-q", "72", "-m", "4", ...webpFrames, "-o", webpPath],
  { stdio: "inherit" },
);

const [mp4Stat, webpStat] = await Promise.all([fs.stat(mp4Path), fs.stat(webpPath)]);
console.log(
  `PASS: demo.mp4 ${(mp4Stat.size / 1024).toFixed(1)} KiB, demo.webp ${(webpStat.size / 1024).toFixed(1)} KiB (${webpFrames.length} frames from ${frames.length} screencast captures)`,
);
await fs.rm(TMP_DIR, { recursive: true, force: true });
