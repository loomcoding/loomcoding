// Dícese de la captura de píxeles de una página servida por ttyd, sin extensión alguna:
// Chrome sin cabeza y el protocolo CDP, esperando a que el contenido llegue por WebSocket.
// Uso: node scripts/visual/capture-web.mjs <url> <salida.png> [espera_ms] [ancho] [alto]
import { spawn } from 'node:child_process';
import { writeFileSync } from 'node:fs';

const [url, out, waitMs = '4000', width = '1000', height = '500'] = process.argv.slice(2);
if (!url || !out) {
  console.error('uso: capture-web.mjs <url> <salida.png> [espera_ms] [ancho] [alto]');
  process.exit(2);
}
const chromePath =
  process.env.CHROME_PATH ?? 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const port = 9333;
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const chrome = spawn(
  chromePath,
  [
    '--headless=new',
    '--disable-gpu',
    `--remote-debugging-port=${port}`,
    `--user-data-dir=${process.env.TEMP ?? '/tmp'}/chrome-cdp-profile`,
    `--window-size=${width},${height}`,
    'about:blank',
  ],
  { stdio: 'ignore' },
);

try {
  let targets = [];
  for (let i = 0; i < 40 && !targets.length; i++) {
    try {
      targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
    } catch {
      await sleep(250);
    }
  }
  const page = targets.find((target) => target.type === 'page');
  if (!page) throw new Error('Chrome no expuso ninguna pestaña por CDP');

  const socket = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((resolve) => (socket.onopen = resolve));
  let next = 0;
  const pending = new Map();
  socket.onmessage = (message) => {
    const reply = JSON.parse(message.data);
    pending.get(reply.id)?.(reply);
  };
  const send = (method, params = {}) =>
    new Promise((resolve) => {
      const id = ++next;
      pending.set(id, resolve);
      socket.send(JSON.stringify({ id, method, params }));
    });

  await send('Page.navigate', { url });
  await sleep(Number(waitMs));
  const shot = await send('Page.captureScreenshot', { format: 'png' });
  writeFileSync(out, Buffer.from(shot.result.data, 'base64'));
  console.log('ok', out);
  socket.close();
} finally {
  chrome.kill();
}
