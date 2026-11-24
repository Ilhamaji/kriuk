// Minimal ASAR extractor: node asar-extract.js <app.asar> <outdir>
const fs = require('fs');
const path = require('path');

const [, , asarPath, outDir] = process.argv;
if (!asarPath || !outDir) {
  console.error('usage: node asar-extract.js <app.asar> <outdir>');
  process.exit(1);
}

const fd = fs.openSync(asarPath, 'r');
const sizeBuf = Buffer.alloc(8);
fs.readSync(fd, sizeBuf, 0, 8, 0);
const headerSize = sizeBuf.readUInt32LE(4);
const headerBuf = Buffer.alloc(headerSize);
fs.readSync(fd, headerBuf, 0, headerSize, 8);
const jsonSize = headerBuf.readUInt32LE(4);
const header = JSON.parse(headerBuf.toString('utf8', 8, 8 + jsonSize));
const dataOffset = 8 + headerSize;

const unpackedDir = asarPath + '.unpacked';
let files = 0, skipped = 0;

function walk(node, rel) {
  for (const [name, entry] of Object.entries(node.files || {})) {
    const p = path.join(rel, name);
    if (entry.files) { walk(entry, p); continue; }
    const target = path.join(outDir, p);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    if (entry.unpacked) {
      const src = path.join(unpackedDir, p);
      if (fs.existsSync(src)) { fs.copyFileSync(src, target); files++; }
      else { skipped++; console.log('MISSING unpacked: ' + p); }
      continue;
    }
    const buf = Buffer.alloc(entry.size);
    fs.readSync(fd, buf, 0, entry.size, dataOffset + Number(entry.offset));
    fs.writeFileSync(target, buf);
    files++;
  }
}

walk(header, '');
fs.closeSync(fd);
console.log(`extracted ${files} files, ${skipped} missing unpacked`);
