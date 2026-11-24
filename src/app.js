/* Kriuk UI — plain DOM, no framework. Runs in the Tauri webview (and in a browser for quick preview). */
const $ = (s) => document.querySelector(s);
const $$ = (s) => Array.from(document.querySelectorAll(s));

const tauri = window.__TAURI__;
const invoke = (cmd, args) => (tauri ? tauri.core.invoke(cmd, args) : Promise.reject(new Error('no tauri')));
const listen = (event, handler) => (tauri ? tauri.event.listen(event, handler) : Promise.resolve(() => {}));

const state = { mic: true, spk: false, aec: false, focus: true, reduction: 'seimbang', eq: 'netral' };

/* ---------- text ---------- */
const TEXT = {
  en: {
    badge: 'Personal',
    hide: 'Hide',
    tab_audio: 'Audio',
    tab_activity: 'Activity',
    tab_settings: 'Settings',
    banner_on_title: 'Your quiet room is ready.',
    banner_on_sub: 'Focus on the conversation, not the noise.',
    banner_off_title: 'Mic noise cancellation is off.',
    banner_off_sub: 'Background sound will be sent as it is.',
    mic: 'Microphone',
    mic_aria: 'Mic noise cancellation',
    mic_on: 'Noise cancellation on',
    mic_off: 'Noise cancellation off',
    mic_default: 'Default microphone (System)',
    reduction: 'Reduction',
    reduction_ringan: 'Light',
    reduction_seimbang: 'Balanced',
    reduction_maksimal: 'Maximum',
    eq: 'Equalizer',
    eq_netral: 'Neutral',
    eq_jelas: 'Clear',
    eq_hangat: 'Warm',
    eq_auto: 'Auto',
    echo: 'Echo cancellation',
    echo_on: 'Speaker echo removed',
    send: 'Send result to',
    spk_default: 'Default speaker (System)',
    off: 'Off',
    test_note: 'Check your level ·',
    test_idle: 'Mic off',
    test_live: 'Mic live',
    test_start: 'Start',
    test_stop: 'Stop',
    activity_title: 'This session',
    activity_sub: 'Temporary test history, no audio stored.',
    activity_empty: 'No mic tests yet.',
    activity_goto: 'Open audio controls',
    settings_title: 'Kriuk preferences',
    focus: 'Automatic focus mode',
    focus_sub: 'Simulated for this session.',
    start_windows: 'Start with Windows',
    start_windows_sub: 'Open Kriuk in the tray when you sign in.',
    close_tray: 'Close to tray',
    close_tray_sub: 'Keep the chain running when the window closes.',
    language: 'Language',
    theme: 'Appearance',
    theme_light: 'Light',
    theme_dark: 'Dark',
    about_title: 'About Kriuk',
    about_body:
      'Everything runs on your device: echo cancellation, RNNoise 48 kHz, then the equalizer. Turn on <em>Send result to</em> to play the result on a speaker — or a virtual audio cable, so other apps get the cleaned sound.',
    hint: 'Click outside the panel to hide it. Processing keeps running in the tray — click the icon to come back.',
    footer_privacy: 'On-device privacy',
    footer_local: 'Local preview',
  },
  id: {
    badge: 'Personal',
    hide: 'Sembunyikan',
    tab_audio: 'Audio',
    tab_activity: 'Aktivitas',
    tab_settings: 'Setelan',
    banner_on_title: 'Ruang tenangmu siap.',
    banner_on_sub: 'Fokus ke obrolan, bukan gangguan.',
    banner_off_title: 'Peredaman mikrofon mati.',
    banner_off_sub: 'Suara sekitar akan ikut terkirim.',
    mic: 'Mikrofon',
    mic_aria: 'Noise cancellation mikrofon',
    mic_on: 'Noise cancellation aktif',
    mic_off: 'Noise cancellation nonaktif',
    mic_default: 'Mikrofon default (Sistem)',
    reduction: 'Peredaman',
    reduction_ringan: 'Ringan',
    reduction_seimbang: 'Seimbang',
    reduction_maksimal: 'Maksimal',
    eq: 'Equalizer',
    eq_netral: 'Netral',
    eq_jelas: 'Jelas',
    eq_hangat: 'Hangat',
    eq_auto: 'Auto',
    echo: 'Echo cancellation',
    echo_on: 'Gema speaker dihapus',
    send: 'Kirim hasil ke',
    spk_default: 'Speaker default (Sistem)',
    off: 'Nonaktif',
    test_note: 'Cek level suara ·',
    test_idle: 'Mic mati',
    test_live: 'Mic aktif',
    test_start: 'Mulai',
    test_stop: 'Hentikan',
    activity_title: 'Aktivitas sesi ini',
    activity_sub: 'Riwayat tes sementara, tanpa penyimpanan audio.',
    activity_empty: 'Belum ada tes mikrofon.',
    activity_goto: 'Buka kontrol audio',
    settings_title: 'Preferensi Kriuk',
    focus: 'Mode fokus otomatis',
    focus_sub: 'Simulasi untuk sesi ini.',
    start_windows: 'Mulai dengan Windows',
    start_windows_sub: 'Buka Kriuk di tray saat kamu masuk.',
    close_tray: 'Tutup ke tray',
    close_tray_sub: 'Rantai audio tetap jalan saat jendela ditutup.',
    language: 'Bahasa',
    theme: 'Tampilan',
    theme_light: 'Terang',
    theme_dark: 'Gelap',
    about_title: 'Tentang Kriuk',
    about_body:
      'Semua diproses di perangkatmu: echo cancellation, RNNoise 48 kHz, lalu equalizer. Nyalakan <em>Kirim hasil ke</em> untuk memutar hasilnya ke speaker — atau ke kabel audio virtual, supaya aplikasi lain memakai suara yang sudah bersih.',
    hint: 'Klik di luar panel untuk menyembunyikan. Pemrosesan tetap jalan di tray — klik ikonnya untuk kembali.',
    footer_privacy: 'Privasi di perangkatmu',
    footer_local: 'Preview lokal',
  },
};

const LANG_KEY = 'kriuk.lang';
const THEME_KEY = 'kriuk.theme';
const MIC_KEY = 'kriuk.mic';
const SPK_KEY = 'kriuk.spk';
const CHAIN_KEY = 'kriuk.chain';
let lang = localStorage.getItem(LANG_KEY) === 'id' ? 'id' : 'en';
const savedTheme = localStorage.getItem(THEME_KEY);
let theme =
  savedTheme === 'dark' || savedTheme === 'light'
    ? savedTheme
    : matchMedia('(prefers-color-scheme: dark)').matches
      ? 'dark'
      : 'light';
// Set before first paint so a dark session does not flash light.
document.documentElement.dataset.theme = theme;
let spkNotice = '';
let aecNotice = '';

const t = (key) => TEXT[lang][key] ?? key;

/* ---------- tabs ---------- */
function selectTab(name) {
  $$('.tab').forEach((t) => {
    const on = t.dataset.tab === name;
    t.classList.toggle('is-active', on);
    t.setAttribute('aria-selected', String(on));
  });
  $$('.view').forEach((v) => v.classList.toggle('is-active', v.id === `view-${name}`));
}
$$('.tab').forEach((t) => t.addEventListener('click', () => selectTab(t.dataset.tab)));
$('#goto-audio').addEventListener('click', () => selectTab('audio'));

/* ---------- toggles ---------- */
function bindToggle(sel, key, onChange) {
  const el = $(sel);
  el.addEventListener('click', () => {
    state[key] = !state[key];
    el.classList.toggle('is-on', state[key]);
    el.setAttribute('aria-checked', String(state[key]));
    if (onChange) onChange(state[key]);
  });
}

/** Every label that depends on state lives here, so a language switch can redraw them all. */
function renderStatus() {
  $('#banner').classList.toggle('is-off', !state.mic);
  $('#banner-title').textContent = t(state.mic ? 'banner_on_title' : 'banner_off_title');
  $('#banner-sub').textContent = t(state.mic ? 'banner_on_sub' : 'banner_off_sub');
  $('#mic-sub').textContent = t(state.mic ? 'mic_on' : 'mic_off');
  $('#echo-sub').textContent = aecNotice || (state.aec ? t('echo_on') : t('off'));
  $('#spk-sub').textContent =
    spkNotice || (state.spk ? spkLabel() || t('spk_default') : t('off'));
  $('#mic-test').textContent = t(testing ? 'test_stop' : 'test_start');
  $('#meter-state').textContent = t(testing ? 'test_live' : 'test_idle');
}

bindToggle('#mic-toggle', 'mic', (on) => {
  renderStatus();
  invoke('set_denoise', { on }).catch(() => {});
  if (!on) stopTest();
});
bindToggle('#focus-toggle', 'focus');

/* ---------- reduction ---------- */
const REDUCTION_LEVEL = { ringan: 0, seimbang: 1, maksimal: 2 };
function setReduction(value) {
  state.reduction = value;
  $$('#reduction button').forEach((b) => b.classList.toggle('is-active', b.dataset.value === value));
  $('#reduction-label').textContent = t(`reduction_${value}`);
  invoke('set_reduction', { level: REDUCTION_LEVEL[value] }).catch(() => {});
}
$$('#reduction button').forEach((b) => b.addEventListener('click', () => setReduction(b.dataset.value)));
$('#reduction-label').addEventListener('click', () => {
  const order = ['ringan', 'seimbang', 'maksimal'];
  setReduction(order[(order.indexOf(state.reduction) + 1) % order.length]);
});

/* ---------- equalizer ---------- */
const EQ_LEVEL = { netral: 0, jelas: 1, hangat: 2, auto: 3 };
function setEq(value) {
  state.eq = value;
  $$('#eq-presets button').forEach((b) => b.classList.toggle('is-active', b.dataset.value === value));
  $('#eq-label').textContent = t(`eq_${value}`);
  invoke('set_eq', { preset: EQ_LEVEL[value] }).catch(() => {});
}
$$('#eq-presets button').forEach((b) => b.addEventListener('click', () => setEq(b.dataset.value)));
$('#eq-label').addEventListener('click', () => {
  const order = ['netral', 'jelas', 'hangat', 'auto'];
  setEq(order[(order.indexOf(state.eq) + 1) % order.length]);
});

/* ---------- language + theme ---------- */
function applyLang() {
  document.documentElement.lang = lang;
  $$('[data-i18n]').forEach((el) => {
    el.textContent = t(el.dataset.i18n);
  });
  $$('[data-i18n-html]').forEach((el) => {
    el.innerHTML = t(el.dataset.i18nHtml);
  });
  $$('[data-i18n-aria]').forEach((el) => {
    const value = t(el.dataset.i18nAria);
    el.setAttribute('aria-label', value);
    if (el.hasAttribute('title')) el.setAttribute('title', value);
  });
  $$('#lang button').forEach((b) => b.classList.toggle('is-active', b.dataset.value === lang));
  $$('#reduction button').forEach((b) => {
    b.textContent = t(`reduction_${b.dataset.value}`);
  });
  $$('#eq-presets button').forEach((b) => {
    b.textContent = t(`eq_${b.dataset.value}`);
  });
  $('#reduction-label').textContent = t(`reduction_${state.reduction}`);
  $('#eq-label').textContent = t(`eq_${state.eq}`);
  invoke('set_language', { lang }).catch(() => {}); // tray menu and tooltip live in Rust
  renderStatus();
}

function setTheme(value) {
  theme = value === 'dark' ? 'dark' : 'light';
  localStorage.setItem(THEME_KEY, theme);
  document.documentElement.dataset.theme = theme;
  $$('#theme button').forEach((b) => b.classList.toggle('is-active', b.dataset.value === theme));
}

$$('#lang button').forEach((b) =>
  b.addEventListener('click', () => {
    lang = b.dataset.value;
    localStorage.setItem(LANG_KEY, lang);
    applyLang();
  })
);
$$('#theme button').forEach((b) => b.addEventListener('click', () => setTheme(b.dataset.value)));

/* ---------- meter ---------- */
const BARS = 12;
const meter = $('#meter');
for (let i = 0; i < BARS; i++) meter.appendChild(document.createElement('i'));
const bars = Array.from(meter.children);

let testing = false;
let peak = 0;
let browser = null; // { ctx, stream, raf } when running outside Tauri

function paintLevel(rms) {
  const db = 20 * Math.log10(rms || 1e-8);
  const lit = Math.max(0, Math.min(BARS, Math.round(((db + 60) / 60) * BARS)));
  bars.forEach((b, i) => b.classList.toggle('on', i < lit));
}

/** Chain state outlives the process: the next launch can come back listening. */
function saveChain() {
  localStorage.setItem(
    CHAIN_KEY,
    JSON.stringify({ on: testing, spk: state.spk, aec: state.aec })
  );
}

function setTesting(on) {
  testing = on;
  $('#mic-test').classList.toggle('is-live', on);
  if (!on) bars.forEach((b) => b.classList.remove('on'));
  renderStatus();
  saveChain();
}

listen('kriuk://level', (event) => {
  if (!testing) return;
  const rms = Number(event.payload) || 0;
  peak = Math.max(peak, rms);
  paintLevel(rms);
});

listen('kriuk://meter-stopped', () => stopTest());

listen('kriuk://meter-error', (event) => {
  console.warn('meter error:', event.payload);
  stopTest();
  $('#meter-state').textContent = lang === 'id' ? 'Mikrofon tidak tersedia' : 'Microphone unavailable';
  listDevices();
});

async function startTest() {
  if (testing) return;
  peak = 0;
  setTesting(true);
  if (tauri) {
    try {
      await invoke('start_meter', { device: $('#mic-device').value || null });
      listDevices();
      if (state.spk) invoke('set_monitor', { on: true }).catch(() => {});
      if (state.aec) invoke('set_aec', { on: true }).catch(() => {});
    } catch (err) {
      console.warn('start_meter failed:', err);
      setTesting(false);
      $('#meter-state').textContent = lang === 'id' ? 'Mikrofon tidak tersedia' : 'Microphone unavailable';
    }
    return;
  }

  try {
    const stream = await navigator.mediaDevices.getUserMedia({
      audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: false },
    });
    const ctx = new AudioContext();
    const analyser = ctx.createAnalyser();
    analyser.fftSize = 2048;
    ctx.createMediaStreamSource(stream).connect(analyser);
    const buf = new Float32Array(analyser.fftSize);
    browser = { ctx, stream, raf: 0 };
    const tick = () => {
      analyser.getFloatTimeDomainData(buf);
      let sum = 0;
      for (let i = 0; i < buf.length; i++) sum += buf[i] * buf[i];
      const rms = Math.sqrt(sum / buf.length);
      peak = Math.max(peak, rms);
      paintLevel(rms);
      browser.raf = requestAnimationFrame(tick);
    };
    tick();
  } catch (err) {
    console.warn('getUserMedia failed:', err);
    setTesting(false);
    $('#meter-state').textContent = lang === 'id' ? 'Mikrofon tidak tersedia' : 'Microphone unavailable';
  }
}

function stopTest() {
  if (!testing) return;
  if (tauri) invoke('stop_meter').catch(() => {});
  if (browser) {
    cancelAnimationFrame(browser.raf);
    browser.stream.getTracks().forEach((t) => t.stop());
    browser.ctx.close();
    browser = null;
  }
  // Routing and echo cancellation need the mic chain; without it they do nothing.
  for (const [key, toggle] of [
    ['spk', '#spk-toggle'],
    ['aec', '#echo-toggle'],
  ]) {
    if (!state[key]) continue;
    state[key] = false;
    $(toggle).classList.remove('is-on');
    $(toggle).setAttribute('aria-checked', 'false');
  }
  setTesting(false);
  if (peak > 0) addActivity(peak);
}

$('#mic-test').addEventListener('click', () => (testing ? stopTest() : startTest()));

/* ---------- output routing ---------- */
function spkLabel() {
  const select = $('#spk-device');
  return select.selectedIndex > 0 ? select.value : '';
}

/* "Speakers (ME6S)" and "Microphone (ME6S)" are one box, so the mic hears the monitor. */
function endpointBox(name) {
  const open = (name || '').indexOf('(');
  const close = (name || '').lastIndexOf(')');
  return (close > open && open >= 0 ? name.slice(open + 1, close) : name || '').trim().toLowerCase();
}

function micLabel() {
  const select = $('#mic-device');
  return select.selectedIndex > 0 ? select.value : '';
}

function monitorFeedsMic() {
  const mic = micLabel();
  const out = spkLabel();
  if (!mic || !out || endpointBox(mic) !== endpointBox(out)) return false;
  // A headset keeps its speaker on your ear, so sharing the box is safe there.
  return !/headset|headphone|earphone/.test(out.toLowerCase());
}

bindToggle('#spk-toggle', 'spk', (on) => {
  spkNotice = '';
  // Echo cancellation needs a speaker that does not carry the processed mic.
  if (on && state.aec && !spkLabel()) {
    state.spk = false;
    $('#spk-toggle').classList.remove('is-on');
    $('#spk-toggle').setAttribute('aria-checked', 'false');
    spkNotice = lang === 'id' ? 'Pilih output lain dulu' : 'Pick a different output first';
    renderStatus();
    saveChain();
    return;
  }
  if (on && monitorFeedsMic()) {
    state.spk = false;
    $('#spk-toggle').classList.remove('is-on');
    $('#spk-toggle').setAttribute('aria-checked', 'false');
    spkNotice = lang === 'id' ? 'Mic ada di perangkat ini' : 'The mic is on this device';
    renderStatus();
    saveChain();
    return;
  }
  renderStatus();
  saveChain();
  invoke('set_monitor', { on }).catch(() => {});
  if (on && !testing) startTest(); // routing only exists while the mic chain runs
});

$('#spk-device').addEventListener('change', () => {
  localStorage.setItem(SPK_KEY, $('#spk-device').value);
  invoke('set_output', { device: spkLabel() || null }).catch(() => {});
  if (state.spk) renderStatus();
});

/* ---------- echo cancellation ---------- */
bindToggle('#echo-toggle', 'aec', (on) => {
  aecNotice = '';
  // The canceller must not hear its own monitor output, or it cancels the voice.
  if (on && state.spk && !spkLabel()) {
    state.aec = false;
    $('#echo-toggle').classList.remove('is-on');
    $('#echo-toggle').setAttribute('aria-checked', 'false');
    aecNotice = lang === 'id' ? 'Pilih output lain dulu' : 'Pick a different output first';
    renderStatus();
    saveChain();
    return;
  }
  renderStatus();
  saveChain();
  invoke('set_aec', { on }).catch(() => {});
  if (on && !testing) startTest(); // the reference comes from the running chain
});

listen('kriuk://aec-error', (event) => {
  console.warn('aec:', event.payload);
  state.aec = false;
  aecNotice =
    lang === 'id'
      ? state.spk
        ? 'Butuh output berbeda'
        : 'Tidak tersedia'
      : state.spk
        ? 'Needs a different output'
        : 'Not available';
  $('#echo-toggle').classList.remove('is-on');
  $('#echo-toggle').setAttribute('aria-checked', 'false');
  renderStatus();
  saveChain();
});

listen('kriuk://monitor', (event) => {
  if (event.payload || !state.spk) return;
  state.spk = false;
  spkNotice = lang === 'id' ? 'Perangkat keluaran tidak tersedia' : 'Output device unavailable';
  $('#spk-toggle').classList.remove('is-on');
  $('#spk-toggle').setAttribute('aria-checked', 'false');
  renderStatus();
});

listen('kriuk://monitor-error', () => {
  state.spk = false;
  spkNotice = lang === 'id' ? 'Mic ada di perangkat ini' : 'The mic is on this device';
  $('#spk-toggle').classList.remove('is-on');
  $('#spk-toggle').setAttribute('aria-checked', 'false');
  renderStatus();
  saveChain();
});

/* ---------- activity ---------- */
const activityList = $('#activity-list');
function addActivity(level) {
  const db = Math.round(20 * Math.log10(level || 1e-8));
  const li = document.createElement('li');
  const when = document.createElement('span');
  when.className = 'when';
  when.textContent = new Date().toLocaleTimeString(lang === 'id' ? 'id-ID' : 'en-US', {
    hour: '2-digit',
    minute: '2-digit',
  });
  const value = document.createElement('span');
  value.className = 'peak';
  value.textContent = `${db} dB`;
  li.append(when, value);
  activityList.prepend(li);
  $('#activity-empty').style.display = 'none';
}

/* ---------- devices ---------- */
async function listDevices() {
  let names = [];
  try {
    names = await invoke('list_devices');
  } catch {
    const devices = await navigator.mediaDevices.enumerateDevices().catch(() => []);
    names = devices.filter((d) => d.kind === 'audioinput').map((d) => d.label || 'Microphone');
  }
  const select = $('#mic-device');
  const keep = select.selectedIndex > 0 ? select.value : localStorage.getItem(MIC_KEY) || '';
  select.length = 1; // keep the "default" option from the markup
  names.forEach((name) => select.add(new Option(name, name)));
  select.selectedIndex = names.includes(keep) ? names.indexOf(keep) + 1 : 0;
}

$('#mic-device').addEventListener('change', () =>
  localStorage.setItem(MIC_KEY, $('#mic-device').value)
);

async function listOutputs() {
  let names = [];
  try {
    names = await invoke('list_outputs');
  } catch {
    return;
  }
  const select = $('#spk-device');
  const keep = select.selectedIndex > 0 ? select.value : localStorage.getItem(SPK_KEY) || '';
  select.length = 1; // keep the "default" option from the markup
  names.forEach((name) => select.add(new Option(name, name)));
  select.selectedIndex = names.includes(keep) ? names.indexOf(keep) + 1 : 0;
}

/**
 * The chain was live when the app last quit: bring it back before the user
 * joins a call, using the devices it remembered. Runs after both lists exist.
 */
async function restoreChain() {
  let saved = {};
  try {
    saved = JSON.parse(localStorage.getItem(CHAIN_KEY) || '{}');
  } catch {
    return;
  }
  if (!tauri || !saved.on) return;
  for (const [key, sel] of [
    ['spk', '#spk-toggle'],
    ['aec', '#echo-toggle'],
  ]) {
    if (!saved[key]) continue;
    state[key] = true;
    $(sel).classList.add('is-on');
    $(sel).setAttribute('aria-checked', 'true');
  }
  invoke('set_output', { device: spkLabel() || null }).catch(() => {});
  renderStatus();
  await startTest();
}

Promise.all([listDevices(), listOutputs()]).then(restoreChain);

// Windows only re-enumerates on request: pick up mics enabled in Control Panel
// without restarting, by refreshing whenever the popover is shown or regains focus.
window.addEventListener('focus', () => {
  listDevices();
  listOutputs();
});
listen('kriuk://shown', async () => {
  listDevices();
  listOutputs();
  // The chain keeps running while the panel is hidden, so trust Rust over the UI's memory.
  if (!testing && (await invoke('meter_active').catch(() => false))) setTesting(true);
});

/* ---------- system ---------- */
const CLOSE_KEY = 'kriuk.closeToTray';
let closeToTray = localStorage.getItem(CLOSE_KEY) !== 'off';

function paintSwitch(el, on) {
  el.classList.toggle('is-on', on);
  el.setAttribute('aria-checked', String(on));
}

const closeEl = $('#close-toggle');
paintSwitch(closeEl, closeToTray);
invoke('set_close_to_tray', { on: closeToTray }).catch(() => {});
closeEl.addEventListener('click', () => {
  closeToTray = !closeToTray;
  paintSwitch(closeEl, closeToTray);
  localStorage.setItem(CLOSE_KEY, closeToTray ? 'on' : 'off');
  invoke('set_close_to_tray', { on: closeToTray }).catch(() => {});
});

// The registry is the source of truth: the entry may have been removed elsewhere.
const autoEl = $('#autostart-toggle');
invoke('get_autostart').then((on) => paintSwitch(autoEl, !!on)).catch(() => {});
autoEl.addEventListener('click', async () => {
  const want = !autoEl.classList.contains('is-on');
  paintSwitch(autoEl, want);
  try {
    await invoke('set_autostart', { on: want });
  } catch (err) {
    console.warn('autostart failed:', err);
    paintSwitch(autoEl, !want);
  }
});

/* ---------- window ---------- */
$('#minimize').addEventListener('click', () => invoke('hide_window').catch(() => {}));
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') invoke('hide_window').catch(() => {});
});

setTheme(theme);
applyLang();
