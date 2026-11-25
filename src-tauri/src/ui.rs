//! The popover, drawn with egui. Every number here comes from `src/styles.css`,
//! the stylesheet the WebView2 build shipped, so the two look the same.

use crate::State;
use egui::{
    Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Id, Rect, Rounding, Sense,
    Stroke, Vec2,
};

pub enum Action {
    Tab(u8),
    Hide,
    Denoise(bool),
    Reduction(u8),
    Eq(u8),
    Aec(bool),
    Spk(bool),
    Mic(Option<String>),
    SpkDevice(Option<String>),
    TestToggle,
    Lang(bool),
    Dark(bool),
    Focus(bool),
    CloseToTray(bool),
    Autostart(bool),
}

#[derive(Clone, Copy)]
struct Pal {
    canvas: Color32,
    surface: Color32,
    white: Color32,
    sage_bg: Color32,
    sage_fg: Color32,
    accent: Color32,
    peach: Color32,
    peri: Color32,
    peri_fg: Color32,
    ink: Color32,
    muted: Color32,
    line: Color32,
    border: Color32,
    track: Color32,
    off: Color32,
    knob: Color32,
    meter_off: Color32,
    banner_off: Color32,
    banner_off_fg: Color32,
    accent_soft: Color32,
}

fn c(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

fn pal(dark: bool) -> Pal {
    if dark {
        Pal {
            canvas: c(0x1B1A18),
            surface: c(0x24231F),
            white: c(0x2E2C27),
            sage_bg: c(0x232B20),
            sage_fg: c(0xA8C68F),
            accent: c(0xEE8A62),
            peach: c(0x33251F),
            peri: c(0x262840),
            peri_fg: c(0x9DA2DC),
            ink: c(0xF3F0EA),
            muted: c(0xA6A099),
            line: c(0x302E29),
            border: c(0x36332E),
            track: c(0x262420),
            off: c(0x4A463F),
            knob: c(0xF0EDE7),
            meter_off: c(0x3D3A34),
            banner_off: c(0x2C2520),
            banner_off_fg: c(0xD2A184),
            accent_soft: c(0x6B4230),
        }
    } else {
        Pal {
            canvas: c(0xFCFBF9),
            surface: c(0xF4F2ED),
            white: c(0xFFFFFF),
            sage_bg: c(0xEFF4E9),
            sage_fg: c(0x5B7A4A),
            accent: c(0xD8683F),
            peach: c(0xFBECE6),
            peri: c(0xEAEBF8),
            peri_fg: c(0x6B6FA8),
            ink: c(0x1C1B1A),
            muted: c(0x6E6A65),
            line: c(0xE8E4DC),
            border: c(0xE6E2DA),
            track: c(0xF1EEE7),
            off: c(0xDDD8CF),
            knob: c(0xFFFFFF),
            meter_off: c(0xDED9CF),
            banner_off: c(0xF6EFEA),
            banner_off_fg: c(0xB07A5E),
            accent_soft: c(0xE7CDBE),
        }
    }
}

fn t(id: bool, key: &str) -> &'static str {
    if id {
        match key {
            "badge" => "Personal",
            "tab_audio" => "Audio",
            "tab_activity" => "Aktivitas",
            "tab_settings" => "Setelan",
            "banner_on_title" => "Ruang tenangmu siap.",
            "banner_on_sub" => "Fokus ke obrolan, bukan gangguan.",
            "banner_off_title" => "Peredaman mikrofon mati.",
            "banner_off_sub" => "Suara sekitar akan ikut terkirim.",
            "mic" => "Mikrofon",
            "mic_on" => "Noise cancellation aktif",
            "mic_off" => "Noise cancellation nonaktif",
            "mic_default" => "Mikrofon default (Sistem)",
            "reduction" => "Peredaman",
            "red_0" => "Ringan",
            "red_1" => "Seimbang",
            "red_2" => "Maksimal",
            "eq" => "Equalizer",
            "eq_0" => "Netral",
            "eq_1" => "Jelas",
            "eq_2" => "Hangat",
            "eq_3" => "Auto",
            "echo" => "Echo cancellation",
            "echo_on" => "Gema speaker dihapus",
            "send" => "Kirim hasil ke",
            "spk_default" => "Speaker default (Sistem)",
            "off" => "Nonaktif",
            "test_note" => "Cek level suara ·",
            "test_idle" => "Mic mati",
            "test_live" => "Mic aktif",
            "test_start" => "Mulai",
            "test_stop" => "Hentikan",
            "activity_title" => "Aktivitas sesi ini",
            "activity_sub" => "Riwayat tes sementara, tanpa penyimpanan audio.",
            "activity_empty" => "Belum ada tes mikrofon.",
            "activity_goto" => "Buka kontrol audio",
            "settings_title" => "Preferensi Kriuk",
            "focus" => "Mode fokus otomatis",
            "focus_sub" => "Simulasi untuk sesi ini.",
            "start_windows" => "Mulai dengan Windows",
            "start_windows_sub" => "Buka Kriuk di tray saat kamu masuk.",
            "close_tray" => "Tutup ke tray",
            "close_tray_sub" => "Rantai audio tetap jalan saat jendela ditutup.",
            "language" => "Bahasa",
            "theme" => "Tampilan",
            "theme_light" => "Terang",
            "theme_dark" => "Gelap",
            "about_title" => "Tentang Kriuk",
            "about_body" => "Semua diproses di perangkatmu: echo cancellation, RNNoise 48 kHz, lalu equalizer. Nyalakan Kirim hasil ke untuk memutar hasilnya ke speaker — atau ke kabel audio virtual, supaya aplikasi lain memakai suara yang sudah bersih.",
            "hint" => "Klik di luar panel untuk menyembunyikan. Pemrosesan tetap jalan di tray — klik ikonnya untuk kembali.",
            "footer_privacy" => "Privasi di perangkatmu",
            "footer_local" => "Pratinjau lokal",
            "notice_same_box" => "Mic ada di perangkat ini",
            "notice_aec" => "Butuh output berbeda",
            "notice_aec_error" => "Tidak tersedia",
            _ => "",
        }
    } else {
        match key {
            "badge" => "Personal",
            "tab_audio" => "Audio",
            "tab_activity" => "Activity",
            "tab_settings" => "Settings",
            "banner_on_title" => "Your quiet room is ready.",
            "banner_on_sub" => "Focus on the conversation, not the noise.",
            "banner_off_title" => "Mic noise cancellation is off.",
            "banner_off_sub" => "Background sound will be sent as it is.",
            "mic" => "Microphone",
            "mic_on" => "Noise cancellation on",
            "mic_off" => "Noise cancellation off",
            "mic_default" => "Default microphone (System)",
            "reduction" => "Reduction",
            "red_0" => "Light",
            "red_1" => "Balanced",
            "red_2" => "Maximum",
            "eq" => "Equalizer",
            "eq_0" => "Neutral",
            "eq_1" => "Clear",
            "eq_2" => "Warm",
            "eq_3" => "Auto",
            "echo" => "Echo cancellation",
            "echo_on" => "Speaker echo removed",
            "send" => "Send result to",
            "spk_default" => "Default speaker (System)",
            "off" => "Off",
            "test_note" => "Check your level ·",
            "test_idle" => "Mic off",
            "test_live" => "Mic live",
            "test_start" => "Start",
            "test_stop" => "Stop",
            "activity_title" => "This session",
            "activity_sub" => "Temporary test history, no audio stored.",
            "activity_empty" => "No mic tests yet.",
            "activity_goto" => "Open audio controls",
            "settings_title" => "Kriuk preferences",
            "focus" => "Automatic focus mode",
            "focus_sub" => "Simulated for this session.",
            "start_windows" => "Start with Windows",
            "start_windows_sub" => "Open Kriuk in the tray when you sign in.",
            "close_tray" => "Close to tray",
            "close_tray_sub" => "Keep the chain running when the window closes.",
            "language" => "Language",
            "theme" => "Appearance",
            "theme_light" => "Light",
            "theme_dark" => "Dark",
            "about_title" => "About Kriuk",
            "about_body" => "Everything runs on your device: echo cancellation, RNNoise 48 kHz, then the equalizer. Turn on Send result to to play the result on a speaker — or a virtual audio cable, so other apps get the cleaned sound.",
            "hint" => "Click outside the panel to hide it. Processing keeps running in the tray — click the icon to come back.",
            "footer_privacy" => "On-device privacy",
            "footer_local" => "Local preview",
            "notice_same_box" => "The mic is on this device",
            "notice_aec" => "Needs a different output",
            "notice_aec_error" => "Not available",
            _ => "",
        }
    }
}

fn tr(id: bool, key: &str) -> String {
    t(id, key).to_string()
}

/// Segoe UI, like the webview used: the default egui font is noticeably wider.
/// The bytes are read once and handed to every panel, because closing and
/// reopening the popover builds a brand new context each time.
pub fn install_fonts(ctx: &egui::Context) {
    use std::sync::OnceLock;
    static FONTS: OnceLock<FontDefinitions> = OnceLock::new();
    let fonts = FONTS.get_or_init(|| {
        let mut fonts = FontDefinitions::default();
        if let Ok(bytes) = std::fs::read("C:\\Windows\\Fonts\\segoeui.ttf") {
            fonts
                .font_data
                .insert("segoe".to_owned(), FontData::from_owned(bytes));
            if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
                list.insert(0, "segoe".to_owned());
            }
        }
        for (name, file) in [("sb", "seguisb.ttf"), ("b", "segoeuib.ttf")] {
            let family = FontFamily::Name(name.into());
            match std::fs::read(format!("C:\\Windows\\Fonts\\{file}")) {
                Ok(bytes) => {
                    fonts
                        .font_data
                        .insert(name.to_owned(), FontData::from_owned(bytes));
                    fonts.families.insert(family, vec![name.to_owned()]);
                }
                Err(_) => {
                    let fallback = fonts.families[&FontFamily::Proportional].clone();
                    fonts.families.insert(family, fallback);
                }
            }
        }
        fonts
    });
    ctx.set_fonts(fonts.clone());
}

fn f(size: f32) -> FontId {
    FontId::proportional(size)
}

fn sb(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("sb".into()))
}

fn b(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("b".into()))
}

pub fn draw(ctx: &egui::Context, st: &mut State) -> Vec<Action> {
    let p = pal(st.dark);
    let mut out = Vec::new();
    // egui adds its own gap between every widget; the CSS used explicit margins,
    // so drop it and let the numbers below be the only spacing.
    ctx.style_mut(|s| s.spacing.item_spacing = Vec2::ZERO);
    let full = ctx.screen_rect();
    // The body leaves 10 px under the panel for the caret that points at the tray.
    let panel = Rect::from_min_max(full.min, egui::pos2(full.right(), full.bottom() - 10.0));
    let bg = ctx.layer_painter(egui::LayerId::background());
    bg.rect_filled(panel, Rounding::same(16.0), p.canvas);
    bg.rect_stroke(panel, Rounding::same(16.0), Stroke::new(1.0_f32, p.border));
    caret(&bg, &p, panel);

    egui::TopBottomPanel::top("head")
        .show_separator_line(false)
        .frame(egui::Frame::none().inner_margin(egui::Margin {
            left: 18.0,
            right: 16.0,
            top: 0.0,
            bottom: 0.0,
        }))
        .exact_height(60.0)
        .show(ctx, |ui| header(ui, &p, st, &mut out));

    egui::TopBottomPanel::top("tabs")
        .show_separator_line(false)
        .frame(egui::Frame::none().inner_margin(egui::Margin {
            left: 8.0,
            right: 8.0,
            top: 0.0,
            bottom: 0.0,
        }))
        .exact_height(44.0)
        .show(ctx, |ui| tabs(ui, &p, st, &mut out));

    egui::TopBottomPanel::bottom("foot")
        .show_separator_line(false)
        .frame(egui::Frame::none().inner_margin(egui::Margin {
            left: 18.0,
            right: 18.0,
            top: 0.0,
            bottom: 10.0,
        }))
        .exact_height(54.0)
        .show(ctx, |ui| footer(ui, &p, st));

    egui::CentralPanel::default()
        .frame(egui::Frame::none().inner_margin(egui::Margin {
            left: 20.0,
            right: 20.0,
            top: 16.0,
            bottom: 8.0,
        }))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .id_salt(st.tab)
                .auto_shrink([false, false])
                .show(ui, |ui| match st.tab {
                    1 => activity(ui, &p, st, &mut out),
                    2 => settings(ui, &p, st, &mut out),
                    _ => audio(ui, &p, st, &mut out),
                });
        });

    out
}

/// The little diamond under the panel: a 16 px square rotated 45°, like the CSS.
fn caret(painter: &egui::Painter, p: &Pal, panel: Rect) {
    let cx = panel.center().x;
    let y = panel.bottom();
    let d = 8.0 * std::f32::consts::SQRT_2;
    let tip = egui::pos2(cx, y + d);
    painter.add(egui::Shape::convex_polygon(
        vec![egui::pos2(cx - d, y), tip, egui::pos2(cx + d, y)],
        p.canvas,
        Stroke::NONE,
    ));
    let stroke = Stroke::new(1.0_f32, p.border);
    painter.line_segment([egui::pos2(cx - d, y), tip], stroke);
    painter.line_segment([tip, egui::pos2(cx + d, y)], stroke);
}

fn header(ui: &mut egui::Ui, p: &Pal, st: &mut State, out: &mut Vec<Action>) {
    let r = ui.max_rect();
    let painter = ui.painter().clone();

    // Logo: 30x26 viewBox at 26x22, five rounded bars.
    let k = 22.0 / 26.0;
    let bars = [8.0f32, 16.0, 24.0, 14.0, 6.0];
    for (i, h) in bars.iter().enumerate() {
        let x = r.left() + 1.5 * k + i as f32 * 6.0 * k;
        painter.rect_filled(
            Rect::from_center_size(
                egui::pos2(x, r.center().y),
                Vec2::new(3.0 * k, h * k),
            ),
            Rounding::same(1.5 * k),
            p.accent,
        );
    }

    let left = r.left() + 26.0 + 9.0;
    let word = painter.layout_no_wrap("kriuk".to_owned(), b(22.0), p.ink);
    let w = word.size().x;
    painter.galley(
        egui::pos2(left, r.center().y - word.size().y / 2.0),
        word,
        p.ink,
    );
    painter.text(
        egui::pos2(left + w + 1.0, r.center().y + 5.0),
        Align2::LEFT_CENTER,
        ".",
        b(22.0),
        p.accent,
    );

    let hide = Rect::from_center_size(egui::pos2(r.right() - 13.0, r.center().y), Vec2::splat(26.0));
    let badge_text = painter.layout_no_wrap(t(st.lang_id, "badge").to_owned(), sb(12.0), p.muted);
    let badge = Rect::from_min_size(
        egui::pos2(
            hide.left() - 8.0 - badge_text.size().x - 20.0,
            r.center().y - 11.0,
        ),
        Vec2::new(badge_text.size().x + 20.0, 22.0),
    );
    painter.rect_filled(badge, Rounding::same(8.0), p.track);
    painter.galley(
        egui::pos2(badge.center().x - badge_text.size().x / 2.0, badge.center().y - badge_text.size().y / 2.0),
        badge_text,
        p.muted,
    );

    let resp = ui.interact(hide, ui.id().with("hide"), Sense::click());
    if resp.hovered() {
        painter.rect_filled(hide, Rounding::same(7.0), p.surface);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    painter.line_segment(
        [
            egui::pos2(hide.center().x - 5.0, hide.center().y),
            egui::pos2(hide.center().x + 5.0, hide.center().y),
        ],
        Stroke::new(1.6_f32, if resp.hovered() { p.ink } else { p.muted }),
    );
    if resp.clicked() {
        out.push(Action::Hide);
    }
}

fn tabs(ui: &mut egui::Ui, p: &Pal, st: &mut State, out: &mut Vec<Action>) {
    let r = ui.max_rect();
    let painter = ui.painter().clone();
    let names = ["tab_audio", "tab_activity", "tab_settings"];
    let w = r.width() / 3.0;
    for (i, key) in names.iter().enumerate() {
        let seg = Rect::from_min_size(
            egui::pos2(r.left() + i as f32 * w, r.top()),
            Vec2::new(w, r.height()),
        );
        let on = st.tab as usize == i;
        let resp = ui.interact(seg, ui.id().with(("tab", i)), Sense::click());
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let color = if on {
            p.accent
        } else if resp.hovered() {
            p.ink
        } else {
            p.muted
        };
        let label = painter.layout_no_wrap(
            t(st.lang_id, key).to_owned(),
            if on { sb(14.0) } else { f(14.0) },
            color,
        );
        let gap = 7.0;
        let icon_w = 17.0;
        let total = icon_w + gap + label.size().x;
        let start = seg.center().x - total / 2.0;
        let mid = egui::pos2(start + icon_w / 2.0, seg.center().y);
        match i {
            0 => icon_audio(&painter, mid, 17.0, color),
            1 => icon_activity(&painter, mid, 17.0, color),
            _ => icon_settings(&painter, mid, 17.0, color),
        }
        painter.galley(
            egui::pos2(
                start + icon_w + gap,
                seg.center().y - label.size().y / 2.0,
            ),
            label,
            color,
        );
        if on {
            painter.rect_filled(
                Rect::from_min_max(
                    egui::pos2(seg.left() + 12.0, r.bottom() - 2.5),
                    egui::pos2(seg.right() - 12.0, r.bottom() + 0.5),
                ),
                Rounding::same(2.0),
                p.accent,
            );
        }
        if resp.clicked() {
            out.push(Action::Tab(i as u8));
        }
    }
    painter.line_segment(
        [
            egui::pos2(r.left() - 8.0, r.bottom() - 0.5),
            egui::pos2(r.right() + 8.0, r.bottom() - 0.5),
        ],
        Stroke::new(1.0_f32, p.line),
    );
}

fn footer(ui: &mut egui::Ui, p: &Pal, st: &mut State) {
    let r = ui.max_rect();
    let painter = ui.painter().clone();
    painter.line_segment(
        [
            egui::pos2(r.left() - 18.0, r.top() + 0.5),
            egui::pos2(r.right() + 18.0, r.top() + 0.5),
        ],
        Stroke::new(1.0_f32, p.line),
    );
    shield(&painter, egui::pos2(r.left() + 7.0, r.center().y), 14.0, p.muted);
    painter.text(
        egui::pos2(r.left() + 21.0, r.center().y),
        Align2::LEFT_CENTER,
        t(st.lang_id, "footer_privacy"),
        f(12.5),
        p.muted,
    );
    painter.text(
        egui::pos2(r.right(), r.center().y),
        Align2::RIGHT_CENTER,
        t(st.lang_id, "footer_local"),
        sb(12.5),
        p.muted,
    );
}

/// Keeps the remembered device in the dropdown while the async enumeration is
/// still running or the device is momentarily absent. Without it the row shows
/// "Default" for the first seconds of every launch, so a remembered pick looks
/// forgotten.
fn remember(items: &mut Vec<String>, saved: Option<&String>) {
    if let Some(name) = saved {
        if !items.iter().any(|d| d == name) {
            items.push(name.clone());
        }
    }
}

fn audio(ui: &mut egui::Ui, p: &Pal, st: &mut State, out: &mut Vec<Action>) {
    let id = st.lang_id;
    banner(ui, p, st);
    ui.add_space(16.0);

    if row(
        ui,
        p,
        Icon::Mic,
        &tr(id, "mic"),
        &tr(id, if st.denoise { "mic_on" } else { "mic_off" }),
        st.denoise,
        "mic",
    ) {
        out.push(Action::Denoise(!st.denoise));
    }

    let mut mics = vec![tr(id, "mic_default")];
    mics.extend(st.devices.iter().cloned());
    remember(&mut mics, st.mic.as_ref());
    let mic_idx = st
        .mic
        .as_ref()
        .and_then(|m| mics.iter().position(|d| d == m))
        .unwrap_or(0);
    if let Some(pick) = select(ui, p, st, 0, &mics, mic_idx) {
        out.push(Action::Mic(if pick == 0 { None } else { Some(mics[pick].clone()) }));
    }

    ui.add_space(12.0);
    label_row(ui, p, t(id, "reduction"), t(id, &format!("red_{}", st.reduction)));
    ui.add_space(8.0);
    let labels = [t(id, "red_0"), t(id, "red_1"), t(id, "red_2")];
    if let Some(i) = segmented(ui, p, "red", &labels, st.reduction as usize, false) {
        out.push(Action::Reduction(i as u8));
    }

    ui.add_space(4.0);
    label_row(ui, p, t(id, "eq"), t(id, &format!("eq_{}", st.eq)));
    ui.add_space(8.0);
    let labels = [t(id, "eq_0"), t(id, "eq_1"), t(id, "eq_2"), t(id, "eq_3")];
    if let Some(i) = segmented(ui, p, "eq", &labels, st.eq as usize, true) {
        out.push(Action::Eq(i as u8));
    }

    ui.add_space(4.0);
    let echo_sub = match st.notice {
        Some("aec_same") => tr(id, "notice_aec"),
        Some("aec_error") => tr(id, "notice_aec_error"),
        _ if st.aec_on => tr(id, "echo_on"),
        _ => tr(id, "off"),
    };
    if row(
        ui,
        p,
        Icon::Shield,
        &tr(id, "echo"),
        &echo_sub,
        st.aec_on,
        "echo",
    ) {
        out.push(Action::Aec(!st.aec_on));
    }

    ui.add_space(2.0);
    divider(ui, p);
    ui.add_space(10.0);

    let spk_sub = match st.notice {
        Some("same_box") => tr(id, "notice_same_box"),
        _ if st.spk_on => st.spk.clone().unwrap_or_else(|| tr(id, "spk_default")),
        _ => tr(id, "off"),
    };
    if row(
        ui,
        p,
        Icon::Speaker,
        &tr(id, "send"),
        &spk_sub,
        st.spk_on,
        "spk",
    ) {
        out.push(Action::Spk(!st.spk_on));
    }

    let mut outs = vec![tr(id, "spk_default")];
    outs.extend(st.outputs.iter().cloned());
    remember(&mut outs, st.spk.as_ref());
    let spk_idx = st
        .spk
        .as_ref()
        .and_then(|m| outs.iter().position(|d| d == m))
        .unwrap_or(0);
    if let Some(pick) = select(ui, p, st, 1, &outs, spk_idx) {
        out.push(Action::SpkDevice(if pick == 0 {
            None
        } else {
            Some(outs[pick].clone())
        }));
    }

    ui.add_space(12.0);
    meter_card(ui, p, st, out);

    if let Some(err) = &st.err {
        ui.add_space(6.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
        ui.painter().text(
            egui::pos2(rect.left(), rect.center().y),
            Align2::LEFT_CENTER,
            err,
            f(12.0),
            p.accent,
        );
    }
}

fn activity(ui: &mut egui::Ui, p: &Pal, st: &mut State, out: &mut Vec<Action>) {
    let id = st.lang_id;
    ui.add_space(2.0);
    view_title(ui, p, t(id, "activity_title"));
    ui.add_space(4.0);
    view_sub(ui, p, t(id, "activity_sub"));
    ui.add_space(4.0);

    if st.tests.is_empty() {
        ui.add_space(36.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
        waveform(&ui.painter().clone(), rect.center(), 42.0, p.muted);
        ui.add_space(10.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            t(id, "activity_empty"),
            f(13.5),
            p.muted,
        );
        ui.add_space(10.0);
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
        let painter = ui.painter().clone();
        let w = ui.painter().layout_no_wrap(t(id, "activity_goto").to_owned(), sb(13.5), p.ink).size().x + 36.0;
        let btn = Rect::from_center_size(rect.center(), Vec2::new(w, 36.0));
        painter.rect_filled(btn, Rounding::same(9.0), p.white);
        painter.rect_stroke(btn, Rounding::same(9.0), Stroke::new(1.0_f32, p.border));
        painter.text(btn.center(), Align2::CENTER_CENTER, t(id, "activity_goto"), sb(13.5), p.ink);
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() {
            out.push(Action::Tab(0));
        }
    } else {
        ui.add_space(10.0);
        for (peak, when) in st.tests.iter().rev() {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 38.0), Sense::hover());
            let painter = ui.painter().clone();
            painter.rect_filled(rect, Rounding::same(10.0), p.surface);
            painter.text(
                egui::pos2(rect.left() + 12.0, rect.center().y),
                Align2::LEFT_CENTER,
                when,
                f(12.5),
                p.muted,
            );
            painter.text(
                egui::pos2(rect.right() - 12.0, rect.center().y),
                Align2::RIGHT_CENTER,
                format!("{peak:.0} dB"),
                sb(13.0),
                p.accent,
            );
            ui.add_space(8.0);
        }
    }
}

fn settings(ui: &mut egui::Ui, p: &Pal, st: &mut State, out: &mut Vec<Action>) {
    let id = st.lang_id;
    ui.add_space(2.0);
    view_title(ui, p, t(id, "settings_title"));
    ui.add_space(10.0);

    if plain_row(ui, p, &tr(id, "focus"), &tr(id, "focus_sub"), st.focus, "focus") {
        out.push(Action::Focus(!st.focus));
    }
    if plain_row(ui, p, &tr(id, "start_windows"), &tr(id, "start_windows_sub"), st.autostart, "auto") {
        out.push(Action::Autostart(!st.autostart));
    }
    if plain_row(ui, p, &tr(id, "close_tray"), &tr(id, "close_tray_sub"), st.close_to_tray, "close") {
        out.push(Action::CloseToTray(!st.close_to_tray));
    }

    ui.add_space(6.0);
    label_row(ui, p, t(id, "language"), "");
    ui.add_space(8.0);
    let langs = ["English", "Bahasa Indonesia"];
    if let Some(i) = segmented(ui, p, "lang", &langs, if st.lang_id { 1 } else { 0 }, false) {
        out.push(Action::Lang(i == 1));
    }

    ui.add_space(16.0);
    label_row(ui, p, t(id, "theme"), "");
    ui.add_space(8.0);
    let themes = [t(id, "theme_light"), t(id, "theme_dark")];
    if let Some(i) = segmented(ui, p, "theme", &themes, if st.dark { 1 } else { 0 }, false) {
        out.push(Action::Dark(i == 1));
    }

    ui.add_space(18.0);
    egui::Frame::none()
        .fill(p.sage_bg)
        .rounding(Rounding::same(12.0))
        .inner_margin(egui::Margin {
            left: 16.0,
            right: 16.0,
            top: 14.0,
            bottom: 14.0,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(t(id, "about_title")).font(sb(14.0)).color(p.ink));
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(t(id, "about_body"))
                    .font(f(13.0))
                    .color(p.muted),
            );
        });

    ui.add_space(18.0);
    ui.label(
        egui::RichText::new(t(id, "hint"))
            .font(f(13.0))
            .color(p.muted),
    );
}

fn view_title(ui: &mut egui::Ui, p: &Pal, text: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
    ui.painter().text(
        egui::pos2(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        text,
        sb(15.0),
        p.ink,
    );
}

fn view_sub(ui: &mut egui::Ui, p: &Pal, text: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
    ui.painter().text(
        egui::pos2(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        text,
        f(13.5),
        p.muted,
    );
}

#[derive(Clone, Copy)]
enum Icon {
    Mic,
    Speaker,
    Shield,
}

fn shield(painter: &egui::Painter, center: egui::Pos2, size: f32, color: Color32) {
    let s = size / 2.0;
    let pts = vec![
        egui::pos2(center.x, center.y - s),
        egui::pos2(center.x + s * 0.9, center.y - s * 0.62),
        egui::pos2(center.x + s * 0.9, center.y + s * 0.1),
        egui::pos2(center.x, center.y + s),
        egui::pos2(center.x - s * 0.9, center.y + s * 0.1),
        egui::pos2(center.x - s * 0.9, center.y - s * 0.62),
    ];
    painter.add(egui::Shape::convex_polygon(
        pts,
        Color32::TRANSPARENT,
        Stroke::new(1.3_f32, color),
    ));
}

fn draw_icon(painter: &egui::Painter, center: egui::Pos2, icon: Icon, color: Color32) {
    let stroke = Stroke::new(1.6_f32, color);
    match icon {
        Icon::Mic => {
            painter.rect_stroke(
                Rect::from_center_size(egui::pos2(center.x, center.y - 2.2), Vec2::new(5.0, 9.0)),
                Rounding::same(2.5),
                stroke,
            );
            let c = egui::pos2(center.x, center.y - 0.5);
            painter.add(egui::Shape::line(
                arc_points(c, 5.5, -0.2, std::f32::consts::PI + 0.2),
                stroke,
            ));
            painter.line_segment(
                [
                    egui::pos2(center.x, center.y + 5.0),
                    egui::pos2(center.x, center.y + 7.0),
                ],
                stroke,
            );
        }
        Icon::Speaker => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(center.x - 6.0, center.y - 3.0),
                    egui::pos2(center.x - 3.0, center.y - 3.0),
                    egui::pos2(center.x + 1.0, center.y - 6.5),
                    egui::pos2(center.x + 1.0, center.y + 6.5),
                    egui::pos2(center.x - 3.0, center.y + 3.0),
                    egui::pos2(center.x - 6.0, center.y + 3.0),
                ],
                color,
                Stroke::NONE,
            ));
            painter.add(egui::Shape::line(
                arc_points(
                    egui::pos2(center.x + 1.0, center.y),
                    5.0,
                    -std::f32::consts::FRAC_PI_3,
                    std::f32::consts::FRAC_PI_3,
                ),
                Stroke::new(1.4_f32, color),
            ));
        }
        Icon::Shield => shield(painter, center, 17.0, color),
    }
}

/// Points along an arc, for the curved parts of the icons.
fn arc_points(center: egui::Pos2, radius: f32, from: f32, to: f32) -> Vec<egui::Pos2> {
    let steps = 12;
    (0..=steps)
        .map(|i| {
            let a = from + (to - from) * i as f32 / steps as f32;
            egui::pos2(center.x + radius * a.cos(), center.y + radius * a.sin())
        })
        .collect()
}

/// Tab glyphs, copied from the SVGs in `src/index.html`.
fn icon_audio(painter: &egui::Painter, center: egui::Pos2, size: f32, color: Color32) {
    let k = size / 20.0;
    let bars = [(2.0f32, 7.0f32, 6.0f32), (6.0, 4.0, 12.0), (10.0, 2.0, 16.0), (14.0, 5.0, 10.0)];
    let origin = egui::pos2(center.x - size / 2.0, center.y - size / 2.0);
    for (x, y, h) in bars {
        painter.rect_filled(
            Rect::from_min_size(
                egui::pos2(origin.x + x * k, origin.y + y * k),
                Vec2::new(1.8 * k, h * k),
            ),
            Rounding::same(0.9 * k),
            color,
        );
    }
}

fn icon_activity(painter: &egui::Painter, center: egui::Pos2, size: f32, color: Color32) {
    let k = size / 20.0;
    let origin = egui::pos2(center.x - size / 2.0, center.y - size / 2.0);
    let p = |x: f32, y: f32| egui::pos2(origin.x + x * k, origin.y + y * k);
    painter.add(egui::Shape::line(
        vec![
            p(2.0, 11.0),
            p(5.0, 11.0),
            p(7.2, 6.0),
            p(10.2, 16.0),
            p(12.8, 9.0),
            p(14.4, 11.0),
            p(18.0, 11.0),
        ],
        Stroke::new(1.6_f32, color),
    ));
}

fn icon_settings(painter: &egui::Painter, center: egui::Pos2, size: f32, color: Color32) {
    let k = size / 20.0;
    let stroke = Stroke::new(1.6_f32, color);
    painter.circle_stroke(center, 3.0 * k, stroke);
    for i in 0..8 {
        let a = std::f32::consts::FRAC_PI_2 * i as f32 / 2.0;
        let (dx, dy) = (a.cos(), a.sin());
        painter.line_segment(
            [
                egui::pos2(center.x + dx * 5.6 * k, center.y + dy * 5.6 * k),
                egui::pos2(center.x + dx * 7.6 * k, center.y + dy * 7.6 * k),
            ],
            stroke,
        );
    }
}

fn waveform(painter: &egui::Painter, center: egui::Pos2, width: f32, color: Color32) {
    let k = width / 40.0;
    let origin = egui::pos2(center.x - width / 2.0, center.y - 12.0 * k);
    let p = |x: f32, y: f32| egui::pos2(origin.x + x * k, origin.y + y * k);
    painter.add(egui::Shape::line(
        vec![
            p(1.0, 13.0),
            p(6.0, 13.0),
            p(9.0, 5.0),
            p(13.0, 21.0),
            p(17.0, 9.0),
            p(20.0, 15.0),
            p(23.0, 12.0),
            p(28.0, 12.0),
        ],
        Stroke::new(1.6_f32, color),
    ));
}

fn banner(ui: &mut egui::Ui, p: &Pal, st: &mut State) {
    let on = st.denoise;
    let id = st.lang_id;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 58.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(
        rect,
        Rounding::same(12.0),
        if on { p.sage_bg } else { p.banner_off },
    );
    let c = egui::pos2(rect.left() + 14.0 + 15.0, rect.center().y);
    let fg = if on { p.sage_fg } else { p.banner_off_fg };
    painter.circle_filled(c, 15.0, if on { p.white } else { p.banner_off });
    let check = Stroke::new(2.0_f32, fg);
    painter.line_segment([egui::pos2(c.x - 4.0, c.y + 0.4), egui::pos2(c.x - 1.2, c.y + 3.2)], check);
    painter.line_segment([egui::pos2(c.x - 1.2, c.y + 3.2), egui::pos2(c.x + 4.4, c.y - 3.0)], check);
    let left = rect.left() + 14.0 + 30.0 + 12.0;
    painter.text(
        egui::pos2(left, rect.center().y - 9.0),
        Align2::LEFT_CENTER,
        t(id, if on { "banner_on_title" } else { "banner_off_title" }),
        sb(14.0),
        p.ink,
    );
    painter.text(
        egui::pos2(left, rect.center().y + 9.0),
        Align2::LEFT_CENTER,
        t(id, if on { "banner_on_sub" } else { "banner_off_sub" }),
        f(12.5),
        p.muted,
    );
}

fn divider(ui: &mut egui::Ui, p: &Pal) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment(
        [
            egui::pos2(rect.left(), rect.center().y),
            egui::pos2(rect.right(), rect.center().y),
        ],
        Stroke::new(1.0_f32, p.line),
    );
}

fn label_row(ui: &mut egui::Ui, p: &Pal, label: &str, value: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 17.0), Sense::hover());
    let painter = ui.painter();
    painter.text(
        egui::pos2(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        label,
        f(13.0),
        p.muted,
    );
    if !value.is_empty() {
        painter.text(
            egui::pos2(rect.right(), rect.center().y),
            Align2::RIGHT_CENTER,
            value,
            sb(13.0),
            p.accent,
        );
    }
}

fn toggle_at(ui: &mut egui::Ui, p: &Pal, rect: Rect, on: bool, id: Id) -> bool {
    let resp = ui.interact(rect, id, Sense::click());
    let t = ui.ctx().animate_bool(id, on);
    let bg = if on { p.accent } else { p.off };
    ui.painter()
        .rect_filled(rect, Rounding::same(rect.height() / 2.0), bg);
    let r = rect.height() / 2.0 - 2.0;
    let x = egui::lerp((rect.left() + r + 2.0)..=(rect.right() - r - 2.0), t);
    ui.painter()
        .circle_filled(egui::pos2(x, rect.center().y), r, p.knob);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

fn row(
    ui: &mut egui::Ui,
    p: &Pal,
    icon: Icon,
    title: &str,
    sub: &str,
    on: bool,
    key: &str,
) -> bool {
    let w = ui.available_width();
    // 4 px top, 40 px text block, 12 px bottom: the row padding from the CSS.
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 56.0), Sense::hover());
    let cy = rect.top() + 24.0;
    let painter = ui.painter().clone();
    let tile = Rect::from_center_size(
        egui::pos2(rect.left() + 17.0, cy),
        Vec2::splat(34.0),
    );
    match icon {
        Icon::Mic => {
            painter.rect_filled(tile, Rounding::same(10.0), p.peach);
            draw_icon(&painter, tile.center(), icon, p.accent);
        }
        Icon::Speaker => {
            painter.rect_filled(tile, Rounding::same(10.0), p.peri);
            draw_icon(&painter, tile.center(), icon, p.peri_fg);
        }
        // The shield sits on the bare canvas, no tile behind it.
        Icon::Shield => draw_icon(&painter, tile.center(), icon, p.muted),
    }
    painter.text(
        egui::pos2(rect.left() + 46.0, cy - 9.5),
        Align2::LEFT_CENTER,
        title,
        sb(14.0),
        p.ink,
    );
    painter.text(
        egui::pos2(rect.left() + 46.0, cy + 9.5),
        Align2::LEFT_CENTER,
        sub,
        f(12.5),
        p.muted,
    );
    let toggle = Rect::from_center_size(
        egui::pos2(rect.right() - 20.0, cy),
        Vec2::new(40.0, 22.0),
    );
    toggle_at(ui, p, toggle, on, ui.id().with(("toggle", key)))
}

fn plain_row(ui: &mut egui::Ui, p: &Pal, title: &str, sub: &str, on: bool, key: &str) -> bool {
    let w = ui.available_width();
    // 4 px top, 40 px text block, 16 px bottom.
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 60.0), Sense::hover());
    let cy = rect.top() + 24.0;
    let painter = ui.painter().clone();
    painter.text(
        egui::pos2(rect.left(), cy - 9.5),
        Align2::LEFT_CENTER,
        title,
        sb(14.0),
        p.ink,
    );
    painter.text(
        egui::pos2(rect.left(), cy + 9.5),
        Align2::LEFT_CENTER,
        sub,
        f(12.5),
        p.muted,
    );
    let toggle = Rect::from_center_size(
        egui::pos2(rect.right() - 20.0, cy),
        Vec2::new(40.0, 22.0),
    );
    toggle_at(ui, p, toggle, on, ui.id().with(("toggle", key)))
}

fn segmented(
    ui: &mut egui::Ui,
    p: &Pal,
    key: &str,
    labels: &[&str],
    sel: usize,
    four: bool,
) -> Option<usize> {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 40.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(rect, Rounding::same(9.0), p.track);
    let n = labels.len();
    let seg = (w - 6.0 - 2.0 * (n as f32 - 1.0)) / n as f32;
    let mut picked = None;
    for i in 0..n {
        let r = Rect::from_min_size(
            egui::pos2(rect.left() + 3.0 + i as f32 * (seg + 2.0), rect.top() + 3.0),
            Vec2::new(seg, 34.0),
        );
        let resp = ui.interact(r, ui.id().with((key, i)), Sense::click());
        let active = i == sel;
        if active {
            painter.rect_filled(r, Rounding::same(7.0), p.white);
            painter.rect_stroke(r, Rounding::same(7.0), Stroke::new(1.0_f32, p.border));
        }
        painter.text(
            r.center(),
            Align2::CENTER_CENTER,
            labels[i],
            if active {
                sb(if four { 12.5 } else { 13.5 })
            } else {
                f(if four { 12.5 } else { 13.5 })
            },
            if active {
                p.accent
            } else if resp.hovered() {
                p.ink
            } else {
                p.muted
            },
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() {
            picked = Some(i);
        }
    }
    picked
}

/// A dropdown that keeps the popover's look: one bordered row, a list under it.
/// One row of a device list, and the padding inside its popup.
const LIST_ROW: f32 = 30.0;
const LIST_PAD: f32 = 6.0;

/// Where the device list goes, and how tall it may be. The panel is a fixed
/// 380x780 window and the popup is drawn outside the layout, so a list that ran
/// past the bottom edge would sit where clicks never arrive: open upward when
/// below is short, and cap the height so the rest can scroll.
fn popup_placement(row: Rect, screen: Rect, count: usize) -> (egui::Pos2, f32) {
    let want = count as f32 * LIST_ROW + LIST_PAD * 2.0;
    let below = screen.bottom() - row.bottom() - 4.0;
    let above = row.top() - screen.top() - 4.0;
    if want <= below || below >= above {
        (
            row.left_bottom() + Vec2::new(0.0, 4.0),
            below.min(want).max(LIST_ROW + LIST_PAD * 2.0),
        )
    } else {
        let h = want.min(above).max(LIST_ROW + LIST_PAD * 2.0);
        (egui::pos2(row.left(), row.top() - 4.0 - h), h)
    }
}

fn select(
    ui: &mut egui::Ui,
    p: &Pal,
    st: &mut State,
    slot: u8,
    items: &[String],
    current: usize,
) -> Option<usize> {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 44.0), Sense::click());
    let painter = ui.painter().clone();
    painter.rect_filled(rect, Rounding::same(10.0), p.white);
    painter.rect_stroke(rect.shrink(0.5), Rounding::same(10.0), Stroke::new(1.0_f32, p.border));
    painter.text(
        egui::pos2(rect.left() + 14.0, rect.center().y),
        Align2::LEFT_CENTER,
        &items[current],
        f(13.5),
        p.ink,
    );
    let chev = egui::pos2(rect.right() - 13.0 - 7.5, rect.center().y);
    let stroke = Stroke::new(1.7_f32, p.muted);
    painter.line_segment([egui::pos2(chev.x - 4.0, chev.y - 2.0), egui::pos2(chev.x, chev.y + 2.0)], stroke);
    painter.line_segment([egui::pos2(chev.x, chev.y + 2.0), egui::pos2(chev.x + 4.0, chev.y - 2.0)], stroke);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let was_open = st.open == Some(slot);
    if resp.clicked() {
        st.open = if was_open { None } else { Some(slot) };
    }
    if st.open != Some(slot) {
        return None;
    }

    let ctx = ui.ctx().clone();
    let mut picked = None;
    let (pos, max_h) = popup_placement(rect, ctx.screen_rect(), items.len());
    let area = egui::Area::new(Id::new(("select", slot)))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .show(&ctx, |ui| {
            egui::Frame::popup(&ctx.style())
                .fill(p.white)
                .stroke(Stroke::new(1.0_f32, p.border))
                .rounding(Rounding::same(10.0))
                .inner_margin(egui::Margin::same(LIST_PAD))
                .show(ui, |ui| {
                    ui.set_width(rect.width() - 12.0);
                    egui::ScrollArea::vertical()
                        .max_height((max_h - LIST_PAD * 2.0).max(LIST_ROW))
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for (i, name) in items.iter().enumerate() {
                                let color = if i == current { p.accent } else { p.ink };
                                let b = egui::Button::new(
                                    egui::RichText::new(name).size(13.0).color(color),
                                )
                                .fill(Color32::TRANSPARENT)
                                .min_size(Vec2::new(ui.available_width(), LIST_ROW));
                                if ui.add(b).clicked() {
                                    picked = Some(i);
                                }
                            }
                        });
                });
        });

    if let Some(i) = picked {
        st.open = None;
        return Some(i);
    }
    let outside = was_open
        && ctx.input(|i| i.pointer.any_click())
        && !ctx
            .input(|i| i.pointer.interact_pos())
            .map(|pos| area.response.rect.contains(pos))
            .unwrap_or(false);
    if outside {
        st.open = None;
    }
    None
}

fn meter_card(ui: &mut egui::Ui, p: &Pal, st: &mut State, out: &mut Vec<Action>) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 83.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(rect, Rounding::same(12.0), p.surface);

    let top = Rect::from_min_size(
        egui::pos2(rect.left() + 14.0, rect.top() + 12.0),
        Vec2::new(w - 28.0, 36.0),
    );

    let label = t(st.lang_id, if st.listening { "test_stop" } else { "test_start" });
    let text = painter.layout_no_wrap(label.to_owned(), sb(13.5), if st.listening { p.accent } else { p.ink });
    let btn = Rect::from_min_size(
        egui::pos2(top.right() - text.size().x - 28.0, top.top()),
        Vec2::new(text.size().x + 28.0, 36.0),
    );
    let resp = ui.interact(btn, ui.id().with("test"), Sense::click());
    painter.rect_filled(btn, Rounding::same(9.0), p.white);
    painter.rect_stroke(
        btn,
        Rounding::same(9.0),
        Stroke::new(1.0_f32, if st.listening { p.accent_soft } else { p.border }),
    );
    painter.galley(
        egui::pos2(
            btn.center().x - text.size().x / 2.0,
            btn.center().y - text.size().y / 2.0,
        ),
        text,
        if st.listening { p.accent } else { p.ink },
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if resp.clicked() {
        out.push(Action::TestToggle);
    }

    draw_icon(
        &painter,
        egui::pos2(top.left() + 8.0, top.center().y),
        Icon::Mic,
        p.muted,
    );

    const BARS: usize = 12;
    let left = top.left() + 27.0;
    let right = btn.left() - 10.0;
    let gap = 2.0;
    let bw = ((right - left) - gap * (BARS as f32 - 1.0)) / BARS as f32;
    let db = 20.0 * (st.level.max(1e-8)).log10();
    let lit = (((db + 60.0) / 60.0) * BARS as f32)
        .round()
        .clamp(0.0, BARS as f32) as usize;
    for i in 0..BARS {
        let x = left + i as f32 * (bw + gap);
        painter.rect_filled(
            Rect::from_center_size(
                egui::pos2(x + bw / 2.0, top.center().y),
                Vec2::new(bw, 10.0),
            ),
            Rounding::same(2.0),
            if i < lit { p.accent } else { p.meter_off },
        );
    }

    let note = format!(
        "{} {}",
        t(st.lang_id, "test_note"),
        t(st.lang_id, if st.listening { "test_live" } else { "test_idle" })
    );
    painter.text(
        egui::pos2(rect.left() + 14.0, rect.top() + 12.0 + 36.0 + 9.0),
        Align2::LEFT_TOP,
        note,
        f(12.0),
        p.muted,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every panel builds a new egui context, so the fonts have to be installed
    /// for each one; a one-shot install left the second context without the
    /// semibold and bold families and panicked on the first frame.
    #[test]
    fn fonts_are_installed_for_every_context() {
        for _ in 0..2 {
            let ctx = egui::Context::default();
            install_fonts(&ctx);
            let out = ctx.run(egui::RawInput::default(), |ctx| {
                let p = ctx.layer_painter(egui::LayerId::background());
                p.text(egui::pos2(8.0, 8.0), Align2::LEFT_TOP, "kriuk", b(22.0), Color32::WHITE);
                p.text(egui::pos2(8.0, 40.0), Align2::LEFT_TOP, "kriuk", sb(12.0), Color32::WHITE);
            });
            assert!(!out.shapes.is_empty());
        }
    }

    /// The saved device must stay selected even before enumeration returns it.
    #[test]
    fn a_remembered_device_stays_in_the_dropdown() {
        let mut items = vec!["Default microphone".to_string(), "Mic A".to_string()];
        remember(&mut items, Some(&"Mic A".to_string()));
        assert_eq!(items.len(), 2, "an already listed device is not duplicated");
        remember(&mut items, Some(&"Mic B".to_string()));
        assert_eq!(items.last().map(String::as_str), Some("Mic B"));
        remember(&mut items, None);
        assert_eq!(items.len(), 3);
    }

    /// A device list longer than the space under the row has to stay inside the
    /// window, or its last items can never be clicked.
    #[test]
    fn a_long_device_list_still_fits_the_window() {
        let screen = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(380.0, 780.0));
        // The output row sits low in the panel, with many outputs below it.
        let row = Rect::from_min_size(egui::pos2(18.0, 600.0), Vec2::new(344.0, 44.0));
        let (pos, max_h) = popup_placement(row, screen, 12);
        assert!(pos.y >= screen.top(), "popup starts above the window");
        assert!(
            pos.y + max_h <= screen.bottom(),
            "popup ends below the window: {} > {}",
            pos.y + max_h,
            screen.bottom()
        );
        assert!(pos.y + max_h <= row.top(), "an upward popup covers its row");

        // A short list with room below still opens downward.
        let (pos, max_h) = popup_placement(row, screen, 2);
        assert!(pos.y >= row.bottom());
        assert!((max_h - (2.0 * LIST_ROW + LIST_PAD * 2.0)).abs() < 0.01);
    }

    /// Drawing the open list is the part a geometry test cannot cover: a scroll
    /// area inside a fixed-size Area has to lay out without panicking.
    #[test]
    fn the_open_device_list_draws() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        let mut st = crate::State::from_saved(crate::settings::Saved::default(), false);
        st.open = Some(0);
        let p = pal(false);
        let items: Vec<String> = (0..14).map(|i| format!("Device {i}")).collect();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_max(
                egui::pos2(0.0, 0.0),
                egui::pos2(380.0, 780.0),
            )),
            ..Default::default()
        };
        let out = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                select(ui, &p, &mut st, 0, &items, 1);
            });
        });
        assert!(!out.shapes.is_empty());
        assert_eq!(st.open, Some(0), "a popup nobody clicked stays open");
    }
}
