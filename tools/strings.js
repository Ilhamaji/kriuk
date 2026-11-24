// Extract printable ASCII + UTF-16LE strings: node strings.js <file> [minLen] [filterRegex]
const fs = require('fs');
const [, , file, minLenArg, filterArg] = process.argv;
const minLen = Number(minLenArg || 6);
const filter = filterArg ? new RegExp(filterArg, 'i') : null;
const buf = fs.readFileSync(file);
const out = new Set();

function push(s) {
  if (s.length < minLen) return;
  if (filter && !filter.test(s)) return;
  out.add(s);
}

let cur = '';
for (let i = 0; i < buf.length; i++) {
  const c = buf[i];
  if (c >= 32 && c < 127) cur += String.fromCharCode(c);
  else { push(cur); cur = ''; }
}
push(cur);

// UTF-16LE
cur = '';
for (let i = 0; i + 1 < buf.length; i += 2) {
  const c = buf[i];
  if (c >= 32 && c < 127 && buf[i + 1] === 0) cur += String.fromCharCode(c);
  else { push(cur); cur = ''; }
}
push(cur);

for (const s of out) console.log(s);
