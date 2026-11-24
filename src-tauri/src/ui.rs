//! The popover, drawn with egui. No webview: the whole panel is a few hundred
//! rectangles and a text layout, so the process stays small and starts at once.

use crate::State;
use egui::{Align2, Color32, FontId, Id, Rect, Rounding, Sense, Stroke, Vec2};

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
    border_strong: Color32,
    track: Color32,
    off: Color32,
    knob: Color32,
    meter_off: Color32,
    banner_off: Color32,
    banner_off_fg: Color32,
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
            border_strong: c(0x45413A),
            track: c(0x262420),
            off: c(0x4A463F),
            knob: c(0xF0EDE7),
            meter_off: c(0x3D3A34),
            banner_off: c(0x2C2520),
            banner_off_fg: c(0xD2A184),
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
            border_strong: c(0xD7D2C8),
            track: c(0xF1EEE7),
            off: c(0xDDD8CF),
            knob: c(0xFFFFFF),
            meter_off: c(0xDED9CF),
            banner_off: c(0xF6EFEA),
            banner_off_fg: c(0xB07A5E),
        }
    }
}

fn t(id: bool, key: &str) -> &'static str {
    if id {
        match key {
            "badge" => "Personal",
            "hide" => "Sembunyikan",
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
            "notice_same_box" => "Mic ada di perangkat ini",
            "notice_aec" => "Butuh output berbeda",
            _ => "",
        }
    } else {
        match key {
            "badge" => "Personal",
            "hide" => "Hide",
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
            "notice_same_box" => "The mic is on this device",
            "notice_aec" => "Needs a different output",
            _ => "",
        }
    }
}

fn tr(id: bool, key: &str) -> String {
    t(id, key).to_string()
}

pub fn draw(ctx: &egui::Context, st: &mut State) -> Vec<Action> {
    let p = pal(st.dark);
    let mut out = Vec::new();
    let full = ctx.screen_rect();
    let bg = ctx.layer_painter(egui::LayerId::background());
    bg.rect_filled(full, Rounding::same(16.0), p.canvas);
    bg.rect_stroke(full, Rounding::same(16.0), Stroke::new(1.0_f32, p.border));

    egui::TopBottomPanel::top("head")
        .frame(
            egui::Frame::none().inner_margin(egui::Margin {
                left: 18.0,
                right: 16.0,
                top: 0.0,
                bottom: 0.0,
            }),
        )
        .exact_height(60.0)
        .show(ctx, |ui| header(ui, &p, st, &mut out));

    egui::TopBottomPanel::top("tabs")
        .frame(egui::Frame::none().inner_margin(egui::Margin {
            left: 8.0,
            right: 8.0,
            top: 0.0,
            bottom: 0.0,
        }))
        .exact_height(40.0)
        .show(ctx, |ui| tabs(ui, &p, st, &mut out));

    egui::TopBottomPanel::bottom("foot")
        .frame(
            egui::Frame::none().inner_margin(egui::Margin {
                left: 20.0,
                right: 20.0,
                top: 0.0,
                bottom: 0.0,
            }),
        )
        .exact_height(38.0)
        .show(ctx, |ui| footer(ui, &p, st));

    egui::CentralPanel::default()
        .frame(egui::Frame::none().inner_margin(egui::Margin {
            left: 20.0,
            right: 20.0,
            top: 4.0,
            bottom: 0.0,
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

fn header(ui: &mut egui::Ui, p: &Pal, st: &mut State, out: &mut Vec<Action>) {
    let r = ui.max_rect();
    let painter = ui.painter().clone();
    let bars = [8.0f32, 16.0, 24.0, 14.0, 6.0];
    for (i, h) in bars.iter().enumerate() {
        let x = r.left() + 1.5 + i as f32 * 6.0;
        painter.rect_filled(
            Rect::from_center_size(egui::pos2(x, r.center().y), Vec2::new(3.0, *h)),
            Rounding::same(1.5),
            p.accent,
        );
    }
    let word = painter.layout_no_wrap("kriuk".to_owned(), FontId::proportional(22.0), p.ink);
    let left = r.left() + 34.0;
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
        FontId::proportional(22.0),
        p.accent,
    );

    let hide = Rect::from_center_size(egui::pos2(r.right() - 13.0, r.center().y), Vec2::splat(26.0));
    let badge = Rect::from_min_size(
        egui::pos2(hide.left() - 8.0 - 62.0, r.center().y - 11.0),
        Vec2::new(62.0, 22.0),
    );
    painter.rect_filled(badge, Rounding::same(8.0), p.surface);
    painter.text(
        badge.center(),
        Align2::CENTER_CENTER,
        t(st.lang_id, "badge"),
        FontId::proportional(12.0),
        p.muted,
    );

    let resp = ui.interact(hide, ui.id().with("hide"), Sense::click());
    if resp.hovered() {
        painter.rect_filled(hide, Rounding::same(8.0), p.surface);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    painter.line_segment(
        [
            egui::pos2(hide.center().x - 5.0, hide.center().y),
            egui::pos2(hide.center().x + 5.0, hide.center().y),
        ],
        Stroke::new(1.6_f32, p.muted),
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
        let seg = Rect::from_min_size(egui::pos2(r.left() + i as f32 * w, r.top()), Vec2::new(w, r.height()));
        let on = st.tab as usize == i;
        let resp = ui.interact(seg, ui.id().with(("tab", i)), Sense::click());
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        painter.text(
            egui::pos2(seg.center().x, seg.center().y - 3.0),
            Align2::CENTER_CENTER,
            t(st.lang_id, key),
            FontId::proportional(14.0),
            if on { p.accent } else { p.muted },
        );
        if on {
            painter.rect_filled(
                Rect::from_center_size(
                    egui::pos2(seg.center().x, r.bottom() - 3.0),
                    Vec2::new(44.0, 2.0),
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
        [egui::pos2(r.left(), r.bottom() - 0.5), egui::pos2(r.right(), r.bottom() - 0.5)],
        Stroke::new(1.0_f32, p.line),
    );
}

fn footer(ui: &mut egui::Ui, p: &Pal, st: &mut State) {
    let r = ui.max_rect();
    let painter = ui.painter().clone();
    painter.line_segment(
        [egui::pos2(r.left(), r.top() + 0.5), egui::pos2(r.right(), r.top() + 0.5)],
        Stroke::new(1.0_f32, p.line),
    );
    let c = egui::pos2(r.left() + 6.0, r.center().y);
    shield(&painter, c, 13.0, p.muted);
    painter.text(
        egui::pos2(r.left() + 18.0, r.center().y),
        Align2::LEFT_CENTER,
        t(st.lang_id, "footer_privacy"),
        FontId::proportional(11.5),
        p.muted,
    );
    painter.text(
        egui::pos2(r.right(), r.center().y),
        Align2::RIGHT_CENTER,
        concat!("v", env!("CARGO_PKG_VERSION")),
        FontId::proportional(11.5),
        p.muted,
    );
}

fn audio(ui: &mut egui::Ui, p: &Pal, st: &mut State, out: &mut Vec<Action>) {
    let id = st.lang_id;
    ui.add_space(10.0);
    banner(ui, p, st);

    ui.add_space(14.0);
    if row(
        ui,
        p,
        Icon::Mic,
        &tr(id, "mic"),
        &tr(id, if st.denoise { "mic_on" } else { "mic_off" }),
        st.denoise,
        40.0,
        "mic",
    ) {
        out.push(Action::Denoise(!st.denoise));
    }
    ui.add_space(8.0);

    let mut mics = vec![tr(id, "mic_default")];
    mics.extend(st.devices.iter().cloned());
    let mic_idx = st
        .mic
        .as_ref()
        .and_then(|m| mics.iter().position(|d| d == m))
        .unwrap_or(0);
    if let Some(pick) = select(ui, p, st, 0, &mics, mic_idx) {
        out.push(Action::Mic(if pick == 0 { None } else { Some(mics[pick].clone()) }));
    }

    ui.add_space(14.0);
    label_row(ui, p, t(id, "reduction"), t(id, &format!("red_{}", st.reduction)));
    ui.add_space(8.0);
    let labels = [t(id, "red_0"), t(id, "red_1"), t(id, "red_2")];
    if let Some(i) = segmented(ui, p, "red", &labels, st.reduction as usize) {
        out.push(Action::Reduction(i as u8));
    }

    ui.add_space(14.0);
    label_row(ui, p, t(id, "eq"), t(id, &format!("eq_{}", st.eq)));
    ui.add_space(8.0);
    let labels = [t(id, "eq_0"), t(id, "eq_1"), t(id, "eq_2"), t(id, "eq_3")];
    if let Some(i) = segmented(ui, p, "eq", &labels, st.eq as usize) {
        out.push(Action::Eq(i as u8));
    }

    ui.add_space(10.0);
    let echo_sub = match st.notice {
        Some("aec_same") => tr(id, "notice_aec"),
        _ if st.aec_on => tr(id, "echo_on"),
        _ => tr(id, "off"),
    };
    if row(ui, p, Icon::Shield, &tr(id, "echo"), &echo_sub, st.aec_on, 40.0, "echo") {
        out.push(Action::Aec(!st.aec_on));
    }

    ui.add_space(6.0);
    divider(ui, p);
    ui.add_space(10.0);

    let spk_sub = match st.notice {
        Some("same_box") => tr(id, "notice_same_box"),
        _ if st.spk_on => st
            .spk
            .clone()
            .unwrap_or_else(|| tr(id, "spk_default")),
        _ => tr(id, "off"),
    };
    if row(ui, p, Icon::Speaker, &tr(id, "send"), &spk_sub, st.spk_on, 40.0, "spk") {
        out.push(Action::Spk(!st.spk_on));
    }
    ui.add_space(8.0);

    let mut outs = vec![tr(id, "spk_default")];
    outs.extend(st.outputs.iter().cloned());
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

    ui.add_space(14.0);
    meter_card(ui, p, st, out);
    ui.add_space(6.0);

    let state_text = tr(id, if st.listening { "test_live" } else { "test_idle" });
    let note = format!("{} {}", t(id, "test_note"), state_text);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
    ui.painter().text(
        egui::pos2(rect.left() + 2.0, rect.center().y),
        Align2::LEFT_CENTER,
        note,
        FontId::proportional(11.5),
        p.muted,
    );
    if let Some(err) = &st.err {
        ui.add_space(4.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
        ui.painter().text(
            egui::pos2(rect.left() + 2.0, rect.center().y),
            Align2::LEFT_CENTER,
            err,
            FontId::proportional(11.5),
            p.accent,
        );
    }
    ui.add_space(14.0);
}

fn activity(ui: &mut egui::Ui, p: &Pal, st: &mut State, out: &mut Vec<Action>) {
    let id = st.lang_id;
    ui.add_space(12.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
    ui.painter().text(
        egui::pos2(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        t(id, "activity_title"),
        FontId::proportional(18.0),
        p.ink,
    );
    ui.add_space(4.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
    ui.painter().text(
        egui::pos2(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        t(id, "activity_sub"),
        FontId::proportional(12.5),
        p.muted,
    );
    ui.add_space(18.0);

    if st.tests.is_empty() {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            t(id, "activity_empty"),
            FontId::proportional(13.0),
            p.muted,
        );
        ui.add_space(12.0);
        if button(ui, p, t(id, "activity_goto"), true) {
            out.push(Action::Tab(0));
        }
    } else {
        for (i, peak) in st.tests.iter().enumerate().rev() {
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::hover());
            let painter = ui.painter().clone();
            painter.line_segment(
                [
                    egui::pos2(rect.left(), rect.bottom()),
                    egui::pos2(rect.right(), rect.bottom()),
                ],
                Stroke::new(1.0_f32, p.line),
            );
            painter.text(
                egui::pos2(rect.left() + 2.0, rect.center().y - 8.0),
                Align2::LEFT_CENTER,
                format!("Mic test {}", i + 1),
                FontId::proportional(13.0),
                p.ink,
            );
            painter.text(
                egui::pos2(rect.left() + 2.0, rect.center().y + 9.0),
                Align2::LEFT_CENTER,
                format!("peak {:.0} dB", peak),
                FontId::proportional(12.0),
                p.muted,
            );
        }
    }
    ui.add_space(14.0);
}

fn settings(ui: &mut egui::Ui, p: &Pal, st: &mut State, out: &mut Vec<Action>) {
    let id = st.lang_id;
    ui.add_space(12.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
    ui.painter().text(
        egui::pos2(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        t(id, "settings_title"),
        FontId::proportional(18.0),
        p.ink,
    );
    ui.add_space(16.0);

    if plain_row(ui, p, &tr(id, "start_windows"), &tr(id, "start_windows_sub"), st.autostart, "auto") {
        out.push(Action::Autostart(!st.autostart));
    }
    if plain_row(ui, p, &tr(id, "close_tray"), &tr(id, "close_tray_sub"), st.close_to_tray, "close") {
        out.push(Action::CloseToTray(!st.close_to_tray));
    }

    ui.add_space(8.0);
    label_row(ui, p, t(id, "language"), "");
    ui.add_space(8.0);
    let langs = ["English", "Bahasa Indonesia"];
    if let Some(i) = segmented(ui, p, "lang", &langs, if st.lang_id { 1 } else { 0 }) {
        out.push(Action::Lang(i == 1));
    }

    ui.add_space(14.0);
    label_row(ui, p, t(id, "theme"), "");
    ui.add_space(8.0);
    let themes = [t(id, "theme_light"), t(id, "theme_dark")];
    if let Some(i) = segmented(ui, p, "theme", &themes, if st.dark { 1 } else { 0 }) {
        out.push(Action::Dark(i == 1));
    }

    ui.add_space(16.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 96.0), Sense::hover());
    ui.painter().rect_filled(rect, Rounding::same(12.0), p.surface);
    let mut about = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(Vec2::new(14.0, 12.0)))
            .layout(egui::Layout::top_down(egui::Align::LEFT)),
    );
    about.label(
        egui::RichText::new(t(id, "about_title"))
            .size(13.0)
            .color(p.ink)
            .strong(),
    );
    about.add_space(4.0);
    about.label(
        egui::RichText::new(t(id, "about_body"))
            .size(12.0)
            .color(p.muted),
    );

    ui.add_space(14.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::hover());
    let mut hint = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::LEFT)),
    );
    hint.label(egui::RichText::new(t(id, "hint")).size(11.5).color(p.muted));
    ui.add_space(14.0);
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
        egui::pos2(center.x + s, center.y - s * 0.6),
        egui::pos2(center.x + s, center.y + s * 0.15),
        egui::pos2(center.x, center.y + s),
        egui::pos2(center.x - s, center.y + s * 0.15),
        egui::pos2(center.x - s, center.y - s * 0.6),
    ];
    painter.add(egui::Shape::convex_polygon(pts, Color32::TRANSPARENT, Stroke::new(1.2_f32, color)));
}

fn draw_icon(painter: &egui::Painter, center: egui::Pos2, icon: Icon, color: Color32) {
    let stroke = Stroke::new(1.4_f32, color);
    match icon {
        Icon::Mic => {
            painter.rect_stroke(
                Rect::from_center_size(
                    egui::pos2(center.x, center.y - 2.0),
                    Vec2::new(6.0, 10.0),
                ),
                Rounding::same(3.0),
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(center.x - 4.5, center.y + 0.5),
                    egui::pos2(center.x + 4.5, center.y + 0.5),
                ],
                Stroke::new(1.0_f32, color),
            );
            painter.line_segment(
                [
                    egui::pos2(center.x, center.y + 4.5),
                    egui::pos2(center.x, center.y + 6.5),
                ],
                Stroke::new(1.2_f32, color),
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
            painter.line_segment(
                [
                    egui::pos2(center.x + 3.5, center.y - 3.0),
                    egui::pos2(center.x + 5.5, center.y),
                ],
                Stroke::new(1.2_f32, color),
            );
            painter.line_segment(
                [
                    egui::pos2(center.x + 5.5, center.y),
                    egui::pos2(center.x + 3.5, center.y + 3.0),
                ],
                Stroke::new(1.2_f32, color),
            );
        }
        Icon::Shield => shield(painter, center, 13.0, color),
    }
}

fn banner(ui: &mut egui::Ui, p: &Pal, st: &mut State) {
    let on = st.denoise;
    let id = st.lang_id;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 54.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(
        rect,
        Rounding::same(12.0),
        if on { p.sage_bg } else { p.banner_off },
    );
    let c = egui::pos2(rect.left() + 28.0, rect.center().y);
    let fg = if on { p.sage_fg } else { p.banner_off_fg };
    painter.circle_filled(c, 15.0, if on { p.sage_bg } else { p.banner_off });
    painter.circle_stroke(c, 15.0, Stroke::new(1.0_f32, fg));
    painter.line_segment([egui::pos2(c.x - 4.0, c.y), egui::pos2(c.x - 1.0, c.y + 3.0)], Stroke::new(1.8_f32, fg));
    painter.line_segment([egui::pos2(c.x - 1.0, c.y + 3.0), egui::pos2(c.x + 4.5, c.y - 3.5)], Stroke::new(1.8_f32, fg));
    painter.text(
        egui::pos2(rect.left() + 52.0, rect.center().y - 8.0),
        Align2::LEFT_CENTER,
        t(id, if on { "banner_on_title" } else { "banner_off_title" }),
        FontId::proportional(14.0),
        p.ink,
    );
    painter.text(
        egui::pos2(rect.left() + 52.0, rect.center().y + 9.0),
        Align2::LEFT_CENTER,
        t(id, if on { "banner_on_sub" } else { "banner_off_sub" }),
        FontId::proportional(12.5),
        p.muted,
    );
}

fn divider(ui: &mut egui::Ui, p: &Pal) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment(
        [egui::pos2(rect.left(), rect.center().y), egui::pos2(rect.right(), rect.center().y)],
        Stroke::new(1.0_f32, p.line),
    );
}

fn label_row(ui: &mut egui::Ui, p: &Pal, label: &str, value: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
    let painter = ui.painter();
    painter.text(
        egui::pos2(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        p.muted,
    );
    if !value.is_empty() {
        painter.text(
            egui::pos2(rect.right(), rect.center().y),
            Align2::RIGHT_CENTER,
            value,
            FontId::proportional(13.0),
            p.accent,
        );
    }
}

fn toggle_at(ui: &mut egui::Ui, p: &Pal, rect: Rect, on: bool, id: Id) -> bool {
    let resp = ui.interact(rect, id, Sense::click());
    let t = ui.ctx().animate_bool(id, on);
    let bg = if on { p.accent } else { p.off };
    ui.painter().rect_filled(rect, Rounding::same(rect.height() / 2.0), bg);
    let r = rect.height() / 2.0 - 2.0;
    let x = egui::lerp((rect.left() + r + 2.0)..=(rect.right() - r - 2.0), t);
    ui.painter().circle_filled(egui::pos2(x, rect.center().y), r, p.knob);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

#[allow(clippy::too_many_arguments)]
fn row(
    ui: &mut egui::Ui,
    p: &Pal,
    icon: Icon,
    title: &str,
    sub: &str,
    on: bool,
    h: f32,
    key: &str,
) -> bool {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let painter = ui.painter().clone();
    let tile = Rect::from_center_size(egui::pos2(rect.left() + 17.0, rect.center().y), Vec2::splat(34.0));
    let (bg, fg) = match icon {
        Icon::Mic => (p.peach, p.accent),
        Icon::Speaker => (p.peri, p.peri_fg),
        Icon::Shield => (p.surface, p.muted),
    };
    painter.rect_filled(tile, Rounding::same(10.0), bg);
    draw_icon(&painter, tile.center(), icon, fg);
    painter.text(
        egui::pos2(rect.left() + 46.0, rect.center().y - 8.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(14.0),
        p.ink,
    );
    painter.text(
        egui::pos2(rect.left() + 46.0, rect.center().y + 9.0),
        Align2::LEFT_CENTER,
        sub,
        FontId::proportional(12.5),
        p.muted,
    );
    let tr = Rect::from_center_size(
        egui::pos2(rect.right() - 20.0, rect.center().y),
        Vec2::new(40.0, 22.0),
    );
    toggle_at(ui, p, tr, on, ui.id().with(("toggle", key)))
}

fn plain_row(ui: &mut egui::Ui, p: &Pal, title: &str, sub: &str, on: bool, key: &str) -> bool {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 48.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.text(
        egui::pos2(rect.left(), rect.center().y - 9.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(14.0),
        p.ink,
    );
    painter.text(
        egui::pos2(rect.left(), rect.center().y + 9.0),
        Align2::LEFT_CENTER,
        sub,
        FontId::proportional(12.5),
        p.muted,
    );
    let tr = Rect::from_center_size(
        egui::pos2(rect.right() - 20.0, rect.center().y),
        Vec2::new(40.0, 22.0),
    );
    toggle_at(ui, p, tr, on, ui.id().with(("toggle", key)))
}

fn segmented(ui: &mut egui::Ui, p: &Pal, key: &str, labels: &[&str], sel: usize) -> Option<usize> {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 34.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(rect, Rounding::same(10.0), p.track);
    let n = labels.len();
    let seg = (w - 6.0) / n as f32;
    let mut picked = None;
    for i in 0..n {
        let r = Rect::from_min_size(
            egui::pos2(rect.left() + 3.0 + i as f32 * seg, rect.top() + 3.0),
            Vec2::new(seg, 28.0),
        );
        let resp = ui.interact(r, ui.id().with((key, i)), Sense::click());
        if i == sel {
            painter.rect_filled(r, Rounding::same(8.0), p.white);
            painter.rect_stroke(r, Rounding::same(8.0), Stroke::new(1.0_f32, p.border));
        }
        painter.text(
            r.center(),
            Align2::CENTER_CENTER,
            labels[i],
            FontId::proportional(13.0),
            if i == sel { p.ink } else { p.muted },
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

fn button(ui: &mut egui::Ui, p: &Pal, label: &str, wide: bool) -> bool {
    let w = if wide { ui.available_width() } else { 74.0 };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 34.0), Sense::click());
    let painter = ui.painter();
    painter.rect_filled(rect, Rounding::same(8.0), p.white);
    painter.rect_stroke(rect, Rounding::same(8.0), Stroke::new(1.0_f32, p.border_strong));
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(13.0),
        p.ink,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// A dropdown that keeps the popover's look: one bordered row, a list under it.
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
    painter.rect_stroke(rect, Rounding::same(10.0), Stroke::new(1.0_f32, p.border_strong));
    painter.text(
        egui::pos2(rect.left() + 14.0, rect.center().y),
        Align2::LEFT_CENTER,
        &items[current],
        FontId::proportional(13.5),
        p.ink,
    );
    let chev = egui::pos2(rect.right() - 16.0, rect.center().y);
    painter.line_segment([egui::pos2(chev.x - 4.0, chev.y - 2.0), chev], Stroke::new(1.5_f32, p.muted));
    painter.line_segment([chev, egui::pos2(chev.x + 4.0, chev.y - 2.0)], Stroke::new(1.5_f32, p.muted));
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
    let area = egui::Area::new(Id::new(("select", slot)))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.left_bottom() + Vec2::new(0.0, 4.0))
        .show(&ctx, |ui| {
            egui::Frame::popup(&ctx.style())
                .fill(p.white)
                .stroke(Stroke::new(1.0_f32, p.border))
                .rounding(Rounding::same(10.0))
                .inner_margin(egui::Margin::same(6.0))
                .show(ui, |ui| {
                    ui.set_width(rect.width() - 12.0);
                    for (i, name) in items.iter().enumerate() {
                        let color = if i == current { p.accent } else { p.ink };
                        let b = egui::Button::new(
                            egui::RichText::new(name).size(13.0).color(color),
                        )
                        .fill(Color32::TRANSPARENT)
                        .min_size(Vec2::new(ui.available_width(), 30.0));
                        if ui.add(b).clicked() {
                            picked = Some(i);
                        }
                    }
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
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 62.0), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(rect, Rounding::same(12.0), p.surface);

    let mic = egui::pos2(rect.left() + 20.0, rect.center().y);
    draw_icon(&painter, mic, Icon::Mic, p.muted);

    let btn = Rect::from_center_size(
        egui::pos2(rect.right() - 49.0, rect.center().y),
        Vec2::new(74.0, 30.0),
    );
    let resp = ui.interact(btn, ui.id().with("test"), Sense::click());
    painter.rect_filled(btn, Rounding::same(8.0), p.white);
    painter.rect_stroke(btn, Rounding::same(8.0), Stroke::new(1.0_f32, p.border_strong));
    painter.text(
        btn.center(),
        Align2::CENTER_CENTER,
        t(st.lang_id, if st.listening { "test_stop" } else { "test_start" }),
        FontId::proportional(13.0),
        p.ink,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if resp.clicked() {
        out.push(Action::TestToggle);
    }

    const BARS: usize = 12;
    let left = mic.x + 16.0;
    let right = btn.left() - 14.0;
    let gap = 3.0;
    let bw = ((right - left) - gap * (BARS as f32 - 1.0)) / BARS as f32;
    let db = 20.0 * (st.level.max(1e-8)).log10();
    let lit = (((db + 60.0) / 60.0) * BARS as f32).round().clamp(0.0, BARS as f32) as usize;
    for i in 0..BARS {
        let x = left + i as f32 * (bw + gap);
        let r = Rect::from_center_size(
            egui::pos2(x + bw / 2.0, rect.center().y),
            Vec2::new(bw, 22.0),
        );
        let on = i < lit;
        painter.rect_filled(
            r,
            Rounding::same(2.0),
            if on { p.accent } else { p.meter_off },
        );
    }
}
