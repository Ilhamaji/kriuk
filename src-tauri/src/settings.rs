use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Everything the panel remembers between launches. The webview used to keep
/// this in localStorage; a native window has no such thing, so it is a file.
#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Saved {
    pub chain: bool,
    pub mic: Option<String>,
    pub spk: Option<String>,
    pub aec_on: bool,
    pub denoise: bool,
    pub reduction: u8,
    pub eq: u8,
    pub lang_id: bool,
    /// None until the user picks a theme; the system theme fills the gap.
    pub dark: Option<bool>,
    pub close_to_tray: bool,
}

impl Default for Saved {
    fn default() -> Self {
        Self {
            chain: false,
            mic: None,
            spk: None,
            aec_on: false,
            denoise: true,
            reduction: 1,
            eq: 0,
            lang_id: false,
            dark: None,
            close_to_tray: true,
        }
    }
}

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(base).join("Kriuk").join("settings.json"))
}

pub fn load() -> Saved {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(saved: &Saved) {
    let Some(file) = path() else { return };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let Ok(text) = serde_json::to_string_pretty(saved) else { return };
    // Write beside the target, then rename over it: a crash mid-write must not
    // leave a truncated file that resets every remembered setting on launch.
    let tmp = file.with_extension("json.tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, &file);
    }
}
