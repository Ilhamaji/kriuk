// Renders the promo page to PNG frames over the Chrome DevTools Protocol.
// Usage: node tools/render-promo.mjs [url] [fps] [duration]
import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';

const EDGE = 'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe';
const PORT = 9333;
const argv = process.argv.slice(2).filter((a) => !a.startsWith('--'));
const URL_ = argv[0] || 'http://127.0.0.1:8125/docs/promo/index.html?render=1';
const FPS = Number(argv[1] || 30);
const DURATION = Number(argv[2] || 15);
const OUT = path.resolve('docs/promo/frames');

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const fresh = process.argv.includes('--fresh');
if (fresh) await rm(OUT, { recursive: true, force: true });
await mkdir(OUT, { recursive: true });

const profile = path.join(os.tmpdir(), 'kriuk-promo-profile');
const browser = spawn(EDGE, [
  '--headless=new',
  '--remote-debugging-port=' + PORT,
  '--window-size=1080,1920',
  '--hide-scrollbars',
  '--no-first-run',
  '--no-default-browser-check',
  '--disable-gpu',
  '--force-device-scale-factor=1',
  '--disable-background-timer-throttling',
  '--disable-backgrounding-occluded-windows',
  '--disable-renderer-backgrounding',
  '--user-data-dir=' + profile,
  'about:blank',
], { stdio: 'ignore' });

let target = null;
for (let i = 0; i < 60 && !target; i++) {
  await sleep(250);
  try {
    const list = await (await fetch('http://127.0.0.1:' + PORT + '/json/list')).json();
    target = list.find((t) => t.type === 'page');
  } catch { /* browser still starting */ }
}
if (!target) {
  browser.kill();
  throw new Error('no devtools target');
}

const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws failed')); });
let seq = 0;
const pending = new Map();
ws.onmessage = (ev) => {
  const m = JSON.parse(ev.data);
  const slot = pending.get(m.id);
  if (!slot) return;
  pending.delete(m.id);
  if (m.error) slot.rej(new Error(JSON.stringify(m.error)));
  else slot.res(m.result);
};
const send = (method, params = {}) => new Promise((res, rej) => {
  const id = ++seq;
  pending.set(id, { res, rej });
  ws.send(JSON.stringify({ id, method, params }));
});

await send('Page.enable');
await send('Runtime.enable');
await send('Emulation.setDeviceMetricsOverride', { width: 1080, height: 1920, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: URL_ });
await sleep(2500);

const evaluate = async (expression) => {
  const r = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.text + ' ' + (r.result && r.result.description || ''));
  return r.result.value;
};

const ready = await evaluate('typeof window.__setTime === "function" && !!document.getElementById("app").contentDocument.getElementById("panel")');
if (!ready) {
  browser.kill();
  throw new Error('promo page not ready (iframe blocked or app missing)');
}

const frames = Math.round(FPS * DURATION);
for (let i = 0; i < frames; i++) {
  const file = path.join(OUT, 'frame-' + String(i).padStart(4, '0') + '.png');
  if (existsSync(file)) continue; /* resume: keep frames from an earlier run */
  const t = i / FPS;
  await evaluate('window.__setTime(' + t.toFixed(4) + ')');
  await sleep(24); /* no rAF wait: background tabs throttle it and the loop hangs */
  const shot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
  await writeFile(file, Buffer.from(shot.data, 'base64'));
  if (i % 30 === 0) process.stdout.write('frame ' + i + '/' + frames + '\n');
}

ws.close();
browser.kill();
console.log('done: ' + frames + ' frames in ' + OUT);
