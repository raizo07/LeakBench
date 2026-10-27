#!/usr/bin/env node
/**
 * render.mjs — frame capture and the final cut.
 *
 *   node render.mjs                 render every frame, score, mux
 *   node render.mjs --still 42.5    one PNG at t=42.5 (for looking at things)
 *   node render.mjs --check         revisit frames and prove seek() is pure
 *   node render.mjs --from 30 --to 40 --no-audio   a slice, for iterating
 */
import http from 'node:http';
import fs from 'node:fs';
import fsp from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';
import puppeteer from 'puppeteer-core';

const ROOT = path.dirname(fileURLToPath(import.meta.url));
const CHROME = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const PORT = 8173;
const argv = process.argv.slice(2);
const flag = (name, fallback = null) => {
  const i = argv.indexOf(name);
  return i === -1 ? fallback : (argv[i + 1] ?? true);
};
const has = (name) => argv.includes(name);

const MODE = has('--check') ? 'check' : has('--still') ? 'still' : 'render';
const STILL = flag('--still');
const FROM = Number(flag('--from', 0));
const TO = Number(flag('--to', 0));
const NO_AUDIO = has('--no-audio');
const KEEP = has('--keep');

const MIME = {
  '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8', '.json': 'application/json',
  '.png': 'image/png', '.jpg': 'image/jpeg', '.woff2': 'font/woff2', '.svg': 'image/svg+xml',
};

function serve() {
  return new Promise((resolve) => {
    const server = http.createServer(async (req, res) => {
      const url = decodeURIComponent(req.url.split('?')[0]);
      const rel = url === '/' ? '/src/index.html' : url;
      const file = path.join(ROOT, rel);
      if (!file.startsWith(ROOT)) { res.writeHead(403).end(); return; }
      try {
        const data = await fsp.readFile(file);
        res.writeHead(200, { 'Content-Type': MIME[path.extname(file)] || 'application/octet-stream' });
        res.end(data);
      } catch {
        res.writeHead(404).end('not found');
      }
    });
    server.listen(PORT, '127.0.0.1', () => resolve(server));
  });
}

async function openPage(browser) {
  const page = await browser.newPage();
  await page.setViewport({ width: 1920, height: 1080, deviceScaleFactor: 1 });
  await page.goto(`http://127.0.0.1:${PORT}/src/index.html`, { waitUntil: 'domcontentloaded' });
  await page.waitForFunction('window.__READY === true', { timeout: 30000 });
  return page;
}

async function still(page, t, file) {
  await page.evaluate((time) => window.seek(time), t);
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  await page.screenshot({ path: file, type: 'png' });
  console.log(`still ${t}s -> ${file}`);
}

async function render(page) {
  const framesDir = path.join(ROOT, 'frames');
  await fsp.rm(framesDir, { recursive: true, force: true });
  await fsp.mkdir(framesDir, { recursive: true });

  const meta = await page.evaluate(() => ({
    frames: window.FRAMES,
    fps: window.__TIMELINE__.meta.fps,
    duration: window.__TIMELINE__.meta.duration,
  }));
  const first = TO > 0 ? Math.floor(FROM * meta.fps) : 0;
  const last = TO > 0 ? Math.min(meta.frames, Math.ceil(TO * meta.fps)) : meta.frames;
  const started = Date.now();

  for (let i = first; i < last; i++) {
    const t = i / meta.fps;
    await page.evaluate((time) => window.seek(time), t);
    const name = path.join(framesDir, `f_${String(i).padStart(5, '0')}.jpg`);
    await page.screenshot({ path: name, type: 'jpeg', quality: 92 });
    if (i % 120 === 0) {
      const rate = (i - first + 1) / ((Date.now() - started) / 1000);
      process.stdout.write(`\r  frame ${i}/${last}  ${rate.toFixed(1)} fps  `);
    }
  }
  process.stdout.write(`\r  ${last - first} frames in ${((Date.now() - started) / 1000).toFixed(1)}s\n`);
  return { first, last, fps: meta.fps };
}

function run(cmd, args) {
  return new Promise((resolve, reject) => {
    const p = spawn(cmd, args, { stdio: ['ignore', 'pipe', 'inherit'] });
    let out = '';
    p.stdout.on('data', (d) => (out += d));
    p.on('close', (code) => (code === 0 ? resolve(out) : reject(new Error(`${cmd} exited ${code}`))));
  });
}

async function mux({ first, last, fps }) {
  await fsp.mkdir(path.join(ROOT, 'out'), { recursive: true });
  const hasAudio = !NO_AUDIO && fs.existsSync(path.join(ROOT, 'audio/score.wav'));
  const out = path.join(ROOT, 'out/leakbench.mp4');
  const args = ['-y', '-framerate', String(fps), '-start_number', String(first),
    '-i', path.join(ROOT, 'frames/f_%05d.jpg')];
  if (hasAudio) {
    if (first > 0) args.push('-ss', String(first / fps));
    args.push('-i', path.join(ROOT, 'audio/score.wav'));
  }
  args.push('-frames:v', String(last - first));
  args.push('-c:v', 'libx264', '-preset', 'slow', '-crf', '17', '-pix_fmt', 'yuv420p');
  if (hasAudio) {
    args.push('-af', 'loudnorm=I=-14:TP=-1.5:LRA=11');
    args.push('-c:a', 'aac', '-b:a', '256k', '-ar', '48000');
  }
  args.push('-shortest', '-movflags', '+faststart', out);
  await run('ffmpeg', args);
  const { size } = await fsp.stat(out);
  console.log(`  ${out}  ${(size / 1024 / 1024).toFixed(1)} MB${hasAudio ? '' : ' (no audio)'}`);
  if (!KEEP) await fsp.rm(path.join(ROOT, 'frames'), { recursive: true, force: true });
}

async function check(browser) {
  const page = await openPage(browser);
  const points = [12.4, 47.3, 88.9];
  let ok = true;
  for (const t of points) {
    await page.evaluate((time) => window.seek(time), t);
    await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
    const a = await page.screenshot({ type: 'png' });
    await page.evaluate((time) => window.seek(time), t - 1.7);
    await page.evaluate((time) => window.seek(time), t);
    await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
    const b = await page.screenshot({ type: 'png' });
    const same = Buffer.compare(a, b) === 0;
    console.log(`  t=${t}s revisited: ${same ? 'identical' : 'DIFFERENT'}`);
    ok = ok && same;
  }
  console.log(ok ? '\n  seek() is pure.\n' : '\n  seek() is not pure.\n');
  process.exitCode = ok ? 0 : 1;
}

(async () => {
  const server = await serve();
  const browser = await puppeteer.launch({
    executablePath: CHROME,
    headless: true,
    args: [
      '--no-sandbox', '--disable-gpu', '--hide-scrollbars', '--force-color-profile=srgb',
      '--disable-lcd-text', '--font-render-hinting=none', '--allow-file-access-from-files',
      '--window-size=1920,1080',
    ],
  });
  try {
    if (MODE === 'check') {
      await check(browser);
    } else {
      const page = await openPage(browser);
      if (MODE === 'still') {
        await still(page, Number(STILL), path.join(ROOT, `out/still-${String(STILL).replace('.', '_')}.png`));
      } else {
        const span = await render(page);
        if (!NO_AUDIO) await mux(span);
        else await mux(span);
      }
    }
  } finally {
    await browser.close();
    server.close();
  }
})();
