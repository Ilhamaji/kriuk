// Generates assets/icon.png, assets/tray.png and assets/icon.ico (PNG-in-ICO).
// Draws the kriuk waveform mark in terracotta. No dependencies.
const fs = require('node:fs');
const path = require('node:path');
const zlib = require('node:zlib');

const ACCENT = [0xd8, 0x68, 0x3f];
// bars from the UI logo: x, height (viewBox 30x26)
const BARS = [
  { x: 0, h: 8 },
  { x: 6, h: 16 },
  { x: 12, h: 24 },
  { x: 18, h: 14 },
  { x: 24, h: 6 },
];
const VIEW_W = 30;
const VIEW_H = 26;
const SS = 4; // supersampling factor

function draw(size) {
  const big = size * SS;
  const px = new Float32Array(big * big); // coverage per pixel
  const scale = big / VIEW_W;

  for (const { x, h } of BARS) {
    const bx = x * scale;
    const bw = 3 * scale;
    const by = ((VIEW_H - h) / 2) * scale;
    const bh = h * scale;
    const r = Math.min(bw / 2, bh / 2);
    for (let y = Math.floor(by); y < Math.ceil(by + bh); y++) {
      for (let xx = Math.floor(bx); xx < Math.ceil(bx + bw); xx++) {
        if (xx < 0 || y < 0 || xx >= big || y >= big) continue;
        // rounded rect coverage
        const cx = Math.min(Math.max(xx + 0.5, bx + r), bx + bw - r);
        const cy = Math.min(Math.max(y + 0.5, by + r), by + bh - r);
        const d = Math.hypot(xx + 0.5 - cx, y + 0.5 - cy);
        const cov = Math.min(1, Math.max(0, r + 0.5 - d));
        if (cov > 0) px[y * big + xx] = Math.max(px[y * big + xx], cov);
      }
    }
  }

  // downsample
  const out = Buffer.alloc(size * size * 4);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      let a = 0;
      for (let sy = 0; sy < SS; sy++) {
        for (let sx = 0; sx < SS; sx++) a += px[(y * SS + sy) * big + (x * SS + sx)];
      }
      a = a / (SS * SS);
      const o = (y * size + x) * 4;
      out[o] = ACCENT[0];
      out[o + 1] = ACCENT[1];
      out[o + 2] = ACCENT[2];
      out[o + 3] = Math.round(Math.min(1, a) * 255);
    }
  }
  return out;
}

const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

function crc32(buf) {
  let c = 0xffffffff;
  for (const b of buf) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}

function png(size) {
  const rgba = draw(size);
  const raw = Buffer.alloc(size * (size * 4 + 1));
  for (let y = 0; y < size; y++) {
    raw[y * (size * 4 + 1)] = 0; // filter: none
    rgba.copy(raw, y * (size * 4 + 1) + 1, y * size * 4, (y + 1) * size * 4);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', zlib.deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

function ico(sizes) {
  const images = sizes.map((s) => ({ s, data: png(s) }));
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2); // type: icon
  header.writeUInt16LE(images.length, 4);
  let offset = 6 + 16 * images.length;
  const entries = images.map(({ s, data }) => {
    const e = Buffer.alloc(16);
    e[0] = s >= 256 ? 0 : s;
    e[1] = s >= 256 ? 0 : s;
    e[2] = 0;
    e[3] = 0;
    e.writeUInt16LE(1, 4);
    e.writeUInt16LE(32, 6);
    e.writeUInt32LE(data.length, 8);
    e.writeUInt32LE(offset, 12);
    offset += data.length;
    return e;
  });
  return Buffer.concat([header, ...entries, ...images.map((i) => i.data)]);
}

const out = path.join(__dirname, '..', 'assets');
fs.mkdirSync(out, { recursive: true });
fs.writeFileSync(path.join(out, 'icon.png'), png(256));
fs.writeFileSync(path.join(out, 'tray.png'), png(32));
fs.writeFileSync(path.join(out, 'icon.ico'), ico([16, 24, 32, 48, 64, 128, 256]));
console.log('icons written to', out);
