// Builds docs/promo.mp4 from docs/promo/frames + caption burn-in + synthesized
// BGM and SFX. No voice-over: the story is carried by the on-screen captions.
// Usage: node tools/build-video.mjs
import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { readdir } from 'node:fs/promises';
import path from 'node:path';

const FF = [
  'C:\\Users\\ihead\\AppData\\Local\\Microsoft\\WinGet\\Packages\\Gyan.FFmpeg_Microsoft.Winget.Source_8wekyb3d8bbwe\\ffmpeg-9.1-full_build\\bin\\ffmpeg.exe',
].find(existsSync) || 'ffmpeg';

const FPS = 30;
const DURATION = 15;
const ROOT = process.cwd();
const FRAMES = path.join(ROOT, 'docs/promo/frames');
const OUT = path.join(ROOT, 'docs/promo.mp4');

const frames = (await readdir(FRAMES)).filter((f) => f.endsWith('.png')).length;
if (frames < FPS * DURATION) throw new Error(`only ${frames}/${FPS * DURATION} frames rendered`);

/* captions: [text, in, out, colour] — colour flips to a light tone on the dark beat */
const captions = [
  ['KRIUK \u2014 WINDOWS TRAY AUDIO', 0.7, 2.4, '0x6E6A65'],
  ['ONE CLICK \u00b7 CLEANER VOICE', 3.55, 5.6, '0x6E6A65'],
  ['ECHO CANCELLED \u00b7 CLEAR EQ', 6.25, 8.4, '0x6E6A65'],
  ['RUNS IN TRAY \u00b7 STARTS WITH WINDOWS', 8.9, 11.3, '0xA6A099'],
];

const draw = captions.map(([text, from, to, color]) =>
  `drawtext=fontfile=C\\\\:/Windows/Fonts/segoeuib.ttf:text='${text}':fontcolor=${color}:fontsize=34:` +
  `x=(w-text_w)/2:y=1672:alpha='clip((t-${from})/0.35\\,0\\,1)*clip((${to}-t)/0.35\\,0\\,1)'`
).join(',');

/* one-shot effects: [lavfi source, start, gain] */
const SFX = [
  ['sine=f=520:d=0.2,afade=t=out:st=0.02:d=0.18', 0.12, 0.5],    // panel lands
  ['sine=f=1500:d=0.06,afade=t=out:st=0.004:d=0.055', 3.4, 0.32], // chain starts
  ['sine=f=1500:d=0.06,afade=t=out:st=0.004:d=0.055', 6.1, 0.3],  // echo on
  ['sine=f=1700:d=0.05,afade=t=out:st=0.004:d=0.045', 7.4, 0.26], // eq clear
  ['sine=f=1100:d=0.08,afade=t=out:st=0.006:d=0.07', 8.6, 0.3],   // theme dark
  ['sine=f=1300:d=0.05,afade=t=out:st=0.004:d=0.045', 9.45, 0.24],// language + settings
  ['sine=f=1250:d=0.05,afade=t=out:st=0.004:d=0.045', 10.5, 0.2], // autostart on
  ['anoisesrc=d=0.9:c=pink:r=44100,highpass=f=500,lowpass=f=7000,afade=t=in:st=0:d=0.3,afade=t=out:st=0.35:d=0.55', 11.7, 0.16], // whoosh into lockup
  ['sine=f=660:d=0.22,afade=t=out:st=0.02:d=0.2', 12.35, 0.28],   // lockup pop
];

/* BGM: a four-chord pad that crossfades every 3.75 s, a soft plucked arp and air.
   Everything is synthesized here, so the track carries no third-party licence. */
const CHORDS = [
  [174.61, 220.0, 261.63, 329.63], // Fmaj7
  [130.81, 164.81, 196.0, 246.94], // Cmaj7
  [196.0, 246.94, 293.66, 329.63], // G6
  [220.0, 261.63, 329.63, 392.0],  // Am7
];
const CHORD_AT = 3.5;

const input = ['-y', '-framerate', String(FPS), '-i', path.join(FRAMES, 'frame-%04d.png')];
const filters = [];
const mix = [];
let next = 1;

const addLavfi = (src, filter, label) => {
  input.push('-f', 'lavfi', '-i', src);
  filters.push(`[${next}:a]${filter}[${label}]`);
  mix.push(`[${label}]`);
  next++;
};

CHORDS.forEach((freqs, i) => {
  const wave = freqs.map((f, k) => `${(0.3 - k * 0.04).toFixed(2)}*sin(2*PI*${f}*t)`).join('+');
  const expr = `(${wave})*(0.8+0.2*sin(2*PI*0.15*t))`;
  addLavfi(
    `aevalsrc='${expr}':d=${CHORD_AT + 0.9}:s=44100`,
    `volume=0.55,afade=t=in:st=0:d=1.4,afade=t=out:st=${CHORD_AT - 0.9}:d=1.8,` +
      `aformat=sample_fmts=fltp:sample_rates=44100:channel_layouts=stereo,` +
      `adelay=${Math.round(i * CHORD_AT * 1000)}|${Math.round(i * CHORD_AT * 1000)}`,
    `pad${i}`
  );
});

addLavfi(
  `aevalsrc='0.06*sin(2*PI*659.25*t)*exp(-7*mod(t\\,0.75))':d=${DURATION}:s=44100`,
  'aformat=sample_fmts=fltp:sample_rates=44100:channel_layouts=stereo',
  'arp'
);
addLavfi(
  `anoisesrc=d=${DURATION}:c=pink:r=44100,lowpass=f=900,volume=0.03`,
  'aformat=sample_fmts=fltp:sample_rates=44100:channel_layouts=stereo',
  'air'
);

for (const [src, at, gain] of SFX) {
  addLavfi(
    `${src},volume=${gain},aformat=sample_fmts=fltp:sample_rates=44100:channel_layouts=stereo`,
    `adelay=${Math.round(at * 1000)}|${Math.round(at * 1000)}`,
    `sfx${next}`
  );
}

const audio =
  `${filters.join(';')};` +
  `${mix.join('')}amix=inputs=${mix.length}:normalize=0:dropout_transition=0,` +
  `alimiter=limit=0.95,loudnorm=I=-16:TP=-1.5:LRA=11,` +
  `afade=t=in:st=0:d=0.5,afade=t=out:st=${DURATION - 1}:d=1[aout]`;

const graph = `[0:v]${draw},format=yuv420p[vout];${audio}`;

const args = [
  ...input,
  '-filter_complex', graph,
  '-map', '[vout]', '-map', '[aout]',
  '-c:v', 'libx264', '-preset', 'slow', '-crf', '18', '-r', String(FPS),
  '-c:a', 'aac', '-b:a', '192k', '-ar', '48000',
  '-t', String(DURATION),
  '-movflags', '+faststart',
  OUT,
];

console.log('mixing ' + mix.length + ' audio lanes');
const run = spawn(FF, args, { stdio: ['ignore', 'inherit', 'inherit'] });
run.on('exit', (code) => {
  if (code === 0) console.log('wrote ' + OUT);
  else { console.error('ffmpeg failed: ' + code); process.exit(code); }
});
