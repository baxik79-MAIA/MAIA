//! MAIA visual primitives and product views. All content comes from the existing store.
use super::*;
use egui::{Align, FontId, Layout, Sense, Stroke, pos2, vec2};

#[derive(Clone, Copy)]
struct Palette {
    canvas: Color32,
    rail: Color32,
    surface: Color32,
    raised: Color32,
    border: Color32,
    text: Color32,
    muted: Color32,
    accent: Color32,
    selected: Color32,
    green: Color32,
}
impl Palette {
    fn new(dark: bool) -> Self {
        let c = Color32::from_rgb;
        if dark {
            Self {
                canvas: c(10, 18, 29),
                rail: c(8, 15, 25),
                surface: c(16, 28, 43),
                raised: c(23, 36, 54),
                border: c(36, 51, 74),
                text: c(234, 239, 252),
                muted: c(151, 168, 199),
                accent: c(136, 145, 255),
                selected: c(36, 43, 86),
                green: c(74, 211, 188),
            }
        } else {
            Self {
                canvas: c(240, 243, 250),
                rail: c(230, 235, 246),
                surface: c(252, 253, 255),
                raised: c(232, 237, 249),
                border: c(204, 213, 233),
                text: c(27, 38, 65),
                muted: c(86, 104, 137),
                accent: c(78, 68, 196),
                selected: c(216, 218, 251),
                green: c(16, 123, 104),
            }
        }
    }
}

// Original vector artwork: one 24-unit, rounded-line icon family; no external assets.
fn icon(ui: &mut egui::Ui, kind: usize, color: Color32, size: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    paint_icon(ui.painter(), r, kind, color);
}
fn paint_icon(p: &egui::Painter, r: egui::Rect, kind: usize, color: Color32) {
    let at = |x: f32, y: f32| {
        pos2(
            r.left() + x * r.width() / 24.,
            r.top() + y * r.height() / 24.,
        )
    };
    let s = Stroke::new(1.6_f32, color);
    let line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([at(a.0, a.1), at(b.0, b.1)], s);
    };
    match kind {
        0 => {
            line((3., 11.), (12., 3.));
            line((12., 3.), (21., 11.));
            line((5., 10.), (5., 21.));
            line((5., 21.), (19., 21.));
            line((19., 21.), (19., 10.));
            line((10., 21.), (10., 15.));
            line((10., 15.), (14., 15.));
            line((14., 15.), (14., 21.));
        }
        1 => {
            for (x, y) in [(3., 3.), (14., 3.), (3., 14.), (14., 14.)] {
                p.rect_stroke(
                    egui::Rect::from_min_max(at(x, y), at(x + 7., y + 7.)),
                    2,
                    s,
                    egui::StrokeKind::Inside,
                );
            }
        }
        2 => {
            p.circle_stroke(at(12., 12.), r.width() * 0.38, s);
            line((12., 6.), (12., 12.));
            line((12., 12.), (16., 14.));
        }
        3 => {
            p.rect_stroke(
                egui::Rect::from_min_max(at(5., 3.), at(19., 21.)),
                2,
                s,
                egui::StrokeKind::Inside,
            );
            for y in [8., 12., 16.] {
                line((8., y), (16., y));
            }
        }
        4 => {
            for (a, b) in [
                ((12., 2.), (15., 9.)),
                ((15., 9.), (22., 12.)),
                ((22., 12.), (15., 15.)),
                ((15., 15.), (12., 22.)),
                ((12., 22.), (9., 15.)),
                ((9., 15.), (2., 12.)),
                ((2., 12.), (9., 9.)),
                ((9., 9.), (12., 2.)),
            ] {
                line(a, b);
            }
        }
        5 => {
            line((4., 12.), (20., 12.));
            line((14., 6.), (20., 12.));
            line((20., 12.), (14., 18.));
        }
        _ => {
            p.circle_stroke(at(12., 12.), r.width() * 0.3, s);
            line((8., 12.), (11., 15.));
            line((11., 15.), (17., 9.));
        }
    }
}
fn mark(ui: &mut egui::Ui, size: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let p = ui.painter();
    for (a, b, c) in [
        (
            vec2(0.12, 0.78),
            vec2(0.34, 0.22),
            Color32::from_rgb(101, 82, 255),
        ),
        (
            vec2(0.34, 0.22),
            vec2(0.58, 0.78),
            Color32::from_rgb(133, 168, 255),
        ),
        (
            vec2(0.58, 0.78),
            vec2(0.79, 0.22),
            Color32::from_rgb(80, 91, 242),
        ),
        (
            vec2(0.79, 0.22),
            vec2(0.98, 0.78),
            Color32::from_rgb(118, 91, 255),
        ),
    ] {
        p.line_segment(
            [r.min + a * size, r.min + b * size],
            Stroke::new(size * 0.17, c),
        );
    }
}
struct MaiaCard;
impl MaiaCard {
    fn show<R>(
        ui: &mut egui::Ui,
        p: Palette,
        height: f32,
        body: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        egui::Frame::NONE
            .fill(p.surface)
            .stroke(Stroke::new(1.0_f32, p.border))
            .corner_radius(12)
            .inner_margin(22)
            .show(ui, |ui| {
                ui.set_min_width((ui.available_width()).max(0.));
                ui.set_min_height(height);
                ui.with_layout(Layout::top_down(Align::LEFT), body).inner
            })
            .inner
    }
}
fn section(ui: &mut egui::Ui, p: Palette, kind: usize, label: &str) {
    ui.horizontal(|ui| {
        icon(ui, kind, p.accent, 21.);
        ui.add_space(5.);
        ui.label(
            RichText::new(label)
                .size(17.)
                .strong()
                .family(egui::FontFamily::Name("maia-semibold".into()))
                .color(p.text),
        );
    });
    ui.add_space(16.);
}
fn chip(ui: &mut egui::Ui, p: Palette, label: &str, green: bool) {
    egui::Frame::NONE
        .fill(if green {
            p.green.gamma_multiply(0.10)
        } else {
            p.selected
        })
        .stroke(Stroke::new(
            1.0_f32,
            if green {
                p.green.gamma_multiply(0.4)
            } else {
                p.border
            },
        ))
        .corner_radius(20)
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.label(
                RichText::new(label)
                    .size(12.)
                    .color(if green { p.green } else { p.accent }),
            );
        });
}
fn button(ui: &mut egui::Ui, p: Palette, label: &str, primary: bool) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).size(14.).color(if primary {
            Color32::WHITE
        } else {
            p.accent
        }))
        .fill(if primary {
            Color32::from_rgb(79, 70, 218)
        } else {
            p.raised
        })
        .stroke(Stroke::new(
            1.0_f32,
            if primary {
                p.accent.gamma_multiply(0.55)
            } else {
                p.border
            },
        ))
        .corner_radius(8)
        .min_size(vec2(0., 36.)),
    )
}
fn meta(ui: &mut egui::Ui, p: Palette, text: impl Into<String>) {
    ui.label(RichText::new(text).size(13.).color(p.muted));
}
fn arrow_button(ui: &mut egui::Ui, p: Palette) -> egui::Response {
    let (r, response) = ui.allocate_exact_size(vec2(40., 36.), Sense::click());
    ui.painter().rect_filled(
        r,
        8,
        if response.hovered() || response.has_focus() {
            p.accent
        } else {
            Color32::from_rgb(79, 70, 218)
        },
    );
    paint_icon(
        ui.painter(),
        egui::Rect::from_center_size(r.center(), vec2(22., 22.)),
        5,
        Color32::WHITE,
    );
    response
}
fn date(value: &str) -> String {
    value.get(..16).unwrap_or(value).replace('T', "  ") + " UTC"
}

impl MaiaDesktop {
    pub(super) fn tr<'a>(&self, pl: &'a str, en: &'a str) -> &'a str {
        if self.language == "pl" { pl } else { en }
    }
    /// Renders localized, Debug-free failure copy for `kind`, computed fresh
    /// every call from `self.language` — never cached at the moment the
    /// outcome was received (Architecture Desk Batch 1 review #1). Timeout
    /// and MalformedResponse get the exact distinct copy the Desk
    /// specified; the remaining kinds share a truthful generic message
    /// rather than invented per-kind copy the Desk never asked for.
    fn failure_copy(&self, kind: Option<BriefingFailureKind>) -> String {
        match kind {
            Some(BriefingFailureKind::Timeout) => self.tr(
                "MAIA lokalna nie odpowiedziała na czas. Model może być przeciążony — spróbuj ponownie.",
                "MAIA Local did not respond in time. The model may be busy — try again.",
            ).to_string(),
            Some(BriefingFailureKind::MalformedResponse) => self.tr(
                "Odpowiedź lokalnego modelu nie przeszła walidacji i nie została zapisana jako briefing.",
                "The local model's response failed validation and was not saved as a briefing.",
            ).to_string(),
            Some(BriefingFailureKind::Unavailable | BriefingFailureKind::ModelUnavailable) => self.tr(
                "MAIA lokalna jest niedostępna. Sprawdź, czy usługa lokalna działa.",
                "MAIA Local is unavailable. Check that the local runtime is running.",
            ).to_string(),
            _ => self.tr(
                "Nie udało się przygotować briefingu. Spróbuj ponownie. Szczegóły znajdziesz w diagnostyce.",
                "The briefing could not be prepared. Try again. See diagnostics for details.",
            ).to_string(),
        }
    }
    fn count(&self, n: usize, pl: [&str; 3], en: [&str; 2]) -> String {
        let word = if self.language == "pl" {
            if n == 1 {
                pl[0]
            } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
                pl[1]
            } else {
                pl[2]
            }
        } else if n == 1 {
            en[0]
        } else {
            en[1]
        };
        format!("{n} {word}")
    }
    fn source_count(&self, n: usize) -> String {
        self.count(n, ["źródło", "źródła", "źródeł"], ["source", "sources"])
    }
    pub(super) fn render(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if i.modifiers.ctrl && i.modifiers.shift {
                if i.key_pressed(egui::Key::L) {
                    self.language = if self.language == "pl" {
                        "en".into()
                    } else {
                        "pl".into()
                    };
                    self.save_preferences();
                }
                if i.key_pressed(egui::Key::D) {
                    self.dark_theme = !self.dark_theme;
                    self.save_preferences();
                }
            }
            if i.modifiers.ctrl {
                for (n, key) in [
                    egui::Key::Num1,
                    egui::Key::Num2,
                    egui::Key::Num3,
                    egui::Key::Num4,
                ]
                .iter()
                .enumerate()
                {
                    if i.key_pressed(*key) {
                        self.navigate_to(n);
                        self.selected_citation = None;
                    }
                }
            }
        });
        if !self.initialized {
            self.reload();
            self.initialized = true;
        }
        if let Some(job) = &self.briefing_job {
            match job.try_recv() {
                Ok(outcome) => {
                    self.briefing_job = None;
                    self.briefing_started = None;
                    // BriefingOutcome carries only typed/semantic data — no
                    // presentation copy crosses the channel (Architecture
                    // Desk Batch 1 review #1). All localized text is
                    // computed here, on the live UI thread, at receive time
                    // (and again every frame while a failure banner is
                    // showing, via failure_copy()), so a language switch can
                    // never leave a stale-language string on screen.
                    match outcome {
                        BriefingOutcome::Succeeded {
                            attempt_id,
                            result_id,
                        } => {
                            // These presentation-state transitions belong
                            // exclusively to the live UI thread — the worker
                            // no longer performs any of them on its
                            // (discarded) clone (Architecture Desk Batch 1
                            // review #10).
                            self.reload();
                            if let Some(entry) = self
                                .recent_requests
                                .iter_mut()
                                .find(|e| e.attempt_id == attempt_id)
                            {
                                entry.state = BriefingRequestState::Completed {
                                    result_id: result_id.clone(),
                                };
                            }
                            self.selected_briefing = Some(result_id);
                            self.selected_citation = None;
                            self.page = 2;
                            self.prompt.clear();
                            self.feedback = None;
                            self.current_failure_kind = None;
                        }
                        BriefingOutcome::Failed {
                            attempt_id,
                            kind,
                            disposition,
                            technical_reason,
                        } => {
                            if let Some(entry) = self
                                .recent_requests
                                .iter_mut()
                                .find(|e| e.attempt_id == attempt_id)
                            {
                                entry.state = BriefingRequestState::Failed {
                                    kind,
                                    disposition,
                                    technical_reason,
                                };
                            }
                            self.feedback = Some("briefing");
                            self.current_failure_kind = Some(kind);
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    // The worker thread panicked or was dropped without
                    // sending an outcome. Not a classified provider/product
                    // failure — Other is truthful here without inventing a
                    // more specific kind we have no evidence for.
                    self.briefing_job = None;
                    self.briefing_started = None;
                    self.feedback = Some("briefing");
                    self.current_failure_kind = Some(BriefingFailureKind::Other);
                    // No attempt_id survives a disconnected channel (the
                    // worker never got to send one back), so this is logged
                    // with an explicit placeholder rather than inventing one.
                    log_failure_diagnostics(
                        "<unknown-disconnected-worker>",
                        BriefingFailureKind::Other,
                        FailureDisposition::Terminal,
                        Some(
                            "worker thread disconnected without sending an outcome (possible panic)",
                        ),
                    );
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
            }
        }
        let p = Palette::new(self.dark_theme);
        let mut v = if self.dark_theme {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        v.panel_fill = p.canvas;
        v.window_fill = p.surface;
        v.extreme_bg_color = p.rail;
        v.faint_bg_color = p.raised;
        v.override_text_color = Some(p.text);
        v.selection.bg_fill = p.selected;
        v.selection.stroke = Stroke::new(1.0_f32, p.accent);
        for w in [&mut v.widgets.inactive, &mut v.widgets.noninteractive] {
            w.bg_fill = p.surface;
            w.weak_bg_fill = p.surface;
            w.bg_stroke = Stroke::new(1.0_f32, p.border);
            w.fg_stroke = Stroke::new(1.0_f32, p.text);
            w.corner_radius = 8.into();
        }
        v.widgets.hovered.bg_fill = p.selected;
        v.widgets.hovered.weak_bg_fill = p.selected;
        v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, p.text);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.accent);
        v.widgets.active = v.widgets.hovered;
        ctx.set_visuals(v);
        ctx.style_mut(|s| {
            s.spacing.item_spacing = vec2(10., 10.);
            s.spacing.button_padding = vec2(14., 8.);
            s.text_styles
                .insert(egui::TextStyle::Body, FontId::proportional(15.));
            s.text_styles
                .insert(egui::TextStyle::Button, FontId::proportional(14.));
            s.text_styles
                .insert(egui::TextStyle::Small, FontId::proportional(13.));
            s.text_styles
                .insert(egui::TextStyle::Heading, FontId::proportional(30.));
        });
        egui::SidePanel::left("maia_rail")
            .exact_width(240.)
            .resizable(false)
            .frame(egui::Frame::NONE.fill(p.rail).inner_margin(18))
            .show(ctx, |ui| {
                ui.add_space(18.);
                ui.horizontal(|ui| {
                    mark(ui, 43.);
                    ui.add_space(10.);
                    ui.label(
                        RichText::new("MAIA")
                            .size(29.)
                            .strong()
                            .family(egui::FontFamily::Name("maia-semibold".into())),
                    );
                });
                ui.add_space(46.);
                meta(ui, p, self.tr("TWÓJ DZIEŃ", "YOUR DAY"));
                ui.add_space(7.);
                for (i, key) in ["today", "workspaces", "history", "sources"]
                    .iter()
                    .enumerate()
                {
                    let label = self.text(key);
                    let (r, response) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 48.), Sense::click());
                    if self.page == i || response.hovered() || response.has_focus() {
                        ui.painter().rect_filled(
                            r,
                            9,
                            if self.page == i { p.selected } else { p.raised },
                        );
                    }
                    if self.page == i {
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(r.min + vec2(0., 12.), vec2(3., 24.)),
                            2,
                            p.accent,
                        );
                    }
                    paint_icon(
                        ui.painter(),
                        egui::Rect::from_min_size(r.min + vec2(15., 13.), vec2(22., 22.)),
                        i,
                        if self.page == i { p.accent } else { p.muted },
                    );
                    ui.painter().text(
                        r.min + vec2(49., 24.),
                        egui::Align2::LEFT_CENTER,
                        label,
                        FontId::proportional(15.),
                        if self.page == i { p.text } else { p.muted },
                    );
                    if response.clicked() {
                        self.navigate_to(i);
                        self.selected_citation = None;
                    }
                }
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.horizontal(|ui| {
                        for lang in ["pl", "en"] {
                            if button(ui, p, &lang.to_uppercase(), self.language == lang).clicked()
                            {
                                self.language = lang.into();
                                self.save_preferences();
                            }
                        }
                        if button(
                            ui,
                            p,
                            if self.dark_theme {
                                self.tr("Jasny", "Light")
                            } else {
                                self.tr("Ciemny", "Dark")
                            },
                            false,
                        )
                        .on_hover_text("Ctrl+Shift+D")
                        .clicked()
                        {
                            self.dark_theme = !self.dark_theme;
                            self.save_preferences();
                        }
                    });
                    ui.add_space(14.);
                    ui.allocate_ui_with_layout(
                        vec2(ui.available_width(), 132.),
                        Layout::top_down(Align::LEFT),
                        |ui| {
                            MaiaCard::show(ui, p, 76., |ui| {
                                ui.label(
                                    RichText::new(self.text("local"))
                                        .strong()
                                        .family(egui::FontFamily::Name("maia-semibold".into()))
                                        .size(15.),
                                );
                                ui.horizontal(|ui| {
                                    let (r, _) =
                                        ui.allocate_exact_size(vec2(8., 8.), Sense::hover());
                                    ui.painter().circle_filled(
                                        r.center(),
                                        3.,
                                        if self.local_available {
                                            p.green
                                        } else {
                                            p.muted
                                        },
                                    );
                                    ui.label(if self.local_available {
                                        self.text("ready")
                                    } else {
                                        self.tr("Niedostępna", "Unavailable")
                                    });
                                });
                                // Previously a hardcoded "Qwen · offline"
                                // literal, unconditional regardless of the
                                // live-probed `local_available` driving the
                                // dot/ready label above (Architecture Desk -
                                // "GO - PROCEED AUTONOMOUSLY" Step 4: MAIA
                                // LOCAL-MODEL DETECTION DEFECT CONFIRMED).
                                meta(
                                    ui,
                                    p,
                                    format!(
                                        "Qwen · {}",
                                        if self.local_available {
                                            self.tr("online", "online")
                                        } else {
                                            self.tr("offline", "offline")
                                        }
                                    ),
                                );
                            });
                        },
                    );
                    ui.add_space(20.);
                    meta(
                        ui,
                        p,
                        self.tr(
                            "Twoja praca. Szersza perspektywa.",
                            "Your work. A wider perspective.",
                        ),
                    );
                });
            });
        egui::SidePanel::right("maia_context")
            .exact_width(if ctx.screen_rect().width() < 1350. {
                285.
            } else {
                330.
            })
            .resizable(false)
            .frame(egui::Frame::NONE.fill(p.rail).inner_margin(20))
            .show(ctx, |ui| {
                self.context(ui, p);
            });
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(p.canvas).inner_margin(egui::Margin::symmetric(32,26))).show(ctx,|ui| {
            self.command(ui,p);
            // An honest wait, not an opaque spinner: on CPU-only hardware a
            // real briefing measured 52.8s warm / 73.8s cold, so the elapsed
            // seconds are shown rather than leaving the user guessing whether
            // anything is happening. No fake percentage is displayed, because
            // the provider is non-streaming and real progress is not knowable
            // until the response arrives.
            if self.briefing_job.is_some(){
                let elapsed=self.briefing_started.map(|s|s.elapsed().as_secs()).unwrap_or(0);
                ui.horizontal(|ui|{
                    ui.spinner();
                    meta(ui,p,self.tr("MAIA przygotowuje briefing ze źródeł…","MAIA is preparing your evidence briefing…"));
                    meta(ui,p,format!("{elapsed}s"));
                });
                if elapsed>=20 {
                    meta(ui,p,self.tr(
                        "Czytanie źródeł zajmuje najwięcej czasu. MAIA pracuje lokalnie na Twoim procesorze.",
                        "Reading your sources takes the longest. MAIA is working locally on your CPU.",
                    ));
                }
            }
            if let Some(message)=self.feedback {
                // "briefing" is rendered via failure_copy(), recomputed from
                // self.current_failure_kind fresh every frame — never a
                // cached string, and never raw Provider(Timeout)/{error:?}
                // (Architecture Desk Batch 1 review #1/#6).
                let text: String = match message {
                    "question" => self.tr("Wpisz pytanie, aby przygotować briefing.", "Enter a question to prepare a briefing.").to_string(),
                    "sources" => self.tr("Dodaj źródło, aby MAIA mogła przygotować briefing.", "Add a source so MAIA can prepare a briefing.").to_string(),
                    "store" => self.tr("Nie można otworzyć obszaru. Sprawdź diagnostykę w panelu Szczegóły.", "Unable to open the workspace. Check diagnostics in Details.").to_string(),
                    "briefing" => self.failure_copy(self.current_failure_kind),
                    _ => self.tr("Nie udało się przygotować briefingu. Spróbuj ponownie. Szczegóły znajdziesz w diagnostyce.", "The briefing could not be prepared. Try again. See diagnostics for details.").to_string(),
                };
                ui.horizontal_wrapped(|ui| {meta(ui,p,text);if button(ui,p,self.tr("Zamknij","Dismiss"),false).clicked(){self.feedback=None;self.current_failure_kind=None;}});
            }
            ui.add_space(30.);
            ui.horizontal(|ui| {ui.vertical(|ui| {ui.heading(RichText::new(self.text(["today","workspaces","history","sources"][self.page])).size(32.).strong().family(egui::FontFamily::Name("maia-semibold".into())));meta(ui,p,self.tr("Twoje źródła. Jasny obraz. Kolejny krok.","Your sources. A clearer picture. What comes next."));});});
            ui.add_space(24.);
            egui::ScrollArea::vertical().id_salt("maia_canvas").show(ui,|ui| {match self.page {0=>self.today(ui,p),1=>self.workspace_view(ui,p),2=>self.history_view(ui,p),_=>self.sources_view(ui,p)}});
        });
        if self.show_import {
            let mut open = true;
            egui::Window::new(self.tr("Dodaj lokalne źródło","Add a local source")).open(&mut open).collapsible(false).resizable(false).default_width(510.).show(ctx,|ui| {
                ui.label(self.tr("Wybierz plik tekstowy .txt lub .md (do 256 KB).","Choose a .txt or .md text file (up to 256 KB)."));
                ui.add(TextEdit::singleline(&mut self.evidence_path).desired_width(f32::INFINITY).hint_text(if self.language=="pl"{"Ścieżka do pliku"}else{"File path"}));
                if button(ui,p,self.text("import"),true).clicked(){self.import_evidence();if self.evidence_path.is_empty(){self.show_import=false;}}
                if !self.evidence_path.is_empty(){meta(ui,p,self.tr("Plik musi istnieć, zawierać tekst UTF-8 i mieć format .txt lub .md.","The file must exist, contain UTF-8 text, and use .txt or .md format."));}
            });
            if !open {
                self.show_import = false;
            }
        }
    }
    fn command(&mut self, ui: &mut egui::Ui, p: Palette) {
        // While a briefing is Processing, both mouse and Enter submission are
        // disabled below — one user gesture creates exactly one attempt
        // (Architecture Desk §3/§8: single-flight, no click+Enter duplicate).
        let processing = self.briefing_job.is_some();
        egui::Frame::NONE
            .fill(p.raised)
            .stroke(Stroke::new(1.0_f32, p.accent.gamma_multiply(0.65)))
            .corner_radius(12)
            .inner_margin(egui::Margin::symmetric(18, 13))
            .show(ui, |ui| {
                ui.add_enabled_ui(!processing, |ui| {
                    ui.horizontal(|ui| {
                        icon(ui, 4, p.accent, 28.);
                        ui.add_space(4.);
                        ui.label(
                            RichText::new(self.tr("Zapytaj MAIA", "Ask MAIA"))
                                .size(18.)
                                .strong()
                                .family(egui::FontFamily::Name("maia-semibold".into())),
                        );
                        let hint = self
                            .tr(
                                "O czym chcesz dziś pracować?",
                                "What would you like to work on today?",
                            )
                            .to_owned();
                        // Reserve the send control's own footprint explicitly
                        // instead of a magic constant, so the visible button
                        // and its clickable region always correspond
                        // (source-confirmed suspect behind the unreliable
                        // send-click defect).
                        let arrow_width = 40.;
                        let gap = 8.;
                        let width = (ui.available_width() - arrow_width - gap).max(90.);
                        let edit = ui.add_sized(
                            vec2(width, 34.),
                            TextEdit::singleline(&mut self.prompt)
                                .frame(false)
                                .font(FontId::proportional(15.))
                                .hint_text(hint),
                        );
                        if ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::K)) {
                            edit.request_focus();
                        }
                        // Warm the local model the moment the user shows intent
                        // to ask, so the 21.02s cold load measured on CPU-only
                        // hardware is paid while they type rather than added to
                        // their wait. Once per session; costs nothing if the
                        // runtime is down or the model is already resident.
                        if !self.warmed_local_model && edit.gained_focus() {
                            self.warmed_local_model = true;
                            crate::warm_local_model();
                        }
                        ui.add_space(gap);
                        let send = arrow_button(ui, p)
                            .on_hover_text(self.tr(
                                "Przygotuj briefing ze źródeł",
                                "Prepare a briefing from sources",
                            ))
                            .clicked();
                        if send
                            || (edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                        {
                            self.ask_maia();
                        }
                    });
                });
            });
    }
    fn today(&mut self, ui: &mut egui::Ui, p: Palette) {
        let two = ui.available_width() > 680.;
        if two {
            ui.columns(2, |col| {
                self.workspace_card(&mut col[0], p);
                self.recent_card(&mut col[1], p);
            });
            ui.add_space(12.);
            ui.columns(2, |col| {
                self.sources_card(&mut col[0], p);
                self.local_card(&mut col[1], p);
            });
        } else {
            self.workspace_card(ui, p);
            self.recent_card(ui, p);
            self.sources_card(ui, p);
            self.local_card(ui, p);
        }
        ui.add_space(24.);
        section(ui, p, 2, self.tr("Ostatnia aktywność", "Recent activity"));
        let recent = self.briefings.first().cloned();
        MaiaCard::show(ui, p, 55., |ui| {
            if let Some(b) = recent {
                ui.horizontal(|ui| {
                    icon(ui, 6, p.green, 24.);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(&b.packet.objective)
                                .strong()
                                .family(egui::FontFamily::Name("maia-semibold".into())),
                        );
                        meta(ui, p, date(b.created_at.as_str()));
                    });
                });
            } else if let Some(a) = self.artifacts.first() {
                ui.horizontal(|ui| {
                    icon(ui, 3, p.accent, 24.);
                    ui.vertical(|ui| {
                        ui.label(format!(
                            "{} {}",
                            self.tr("Dodano źródło:", "Source added:"),
                            a.name().as_str()
                        ));
                        meta(ui, p, date(a.created_at().as_str()));
                    });
                });
            } else {
                ui.label(self.tr(
                    "Wszystko zaczyna się od źródła",
                    "Every insight starts with a source",
                ));
                meta(
                    ui,
                    p,
                    self.tr(
                        "Dodaj dokument, a MAIA pomoże uporządkować jego treść.",
                        "Add a document and let MAIA help make sense of it.",
                    ),
                );
            }
        });
    }
    fn workspace_card(&mut self, ui: &mut egui::Ui, p: Palette) {
        MaiaCard::show(ui, p, 210., |ui| {
            section(ui, p, 1, self.tr("Obszar roboczy", "Current workspace"));
            ui.label(
                RichText::new(self.tr("Mój obszar roboczy", "My workspace"))
                    .size(24.)
                    .strong()
                    .family(egui::FontFamily::Name("maia-semibold".into())),
            );
            meta(
                ui,
                p,
                format!(
                    "{} · {}",
                    self.source_count(self.artifacts.len()),
                    self.count(
                        self.briefings.len(),
                        ["briefing", "briefingi", "briefingów"],
                        ["briefing", "briefings"]
                    )
                ),
            );
            ui.add_space(8.);
            if let Some(b) = self.briefings.first() {
                meta(
                    ui,
                    p,
                    format!(
                        "{} {}",
                        self.tr("Aktywność:", "Activity:"),
                        date(b.created_at.as_str())
                    ),
                );
            } else if let Some(a) = self.artifacts.first() {
                meta(
                    ui,
                    p,
                    format!(
                        "{} {}",
                        self.tr("Aktywność:", "Activity:"),
                        date(a.created_at().as_str())
                    ),
                );
            } else {
                meta(
                    ui,
                    p,
                    self.tr("Gotowy na Twoje dokumenty", "Ready for your documents"),
                );
            }
            ui.add_space(18.);
            if button(ui, p, self.tr("Kontynuuj pracę", "Continue working"), true).clicked() {
                self.navigate_to(1);
            }
        });
    }
    fn recent_card(&mut self, ui: &mut egui::Ui, p: Palette) {
        MaiaCard::show(ui, p, 210., |ui| {
            section(ui, p, 2, self.tr("Ostatnie briefingi", "Recent briefings"));
            if let Some(b) = self.briefings.first().cloned() {
                ui.label(
                    RichText::new(truncate(&b.packet.objective, 110))
                        .size(18.)
                        .strong()
                        .family(egui::FontFamily::Name("maia-semibold".into())),
                );
                meta(ui, p, date(b.created_at.as_str()));
                meta(
                    ui,
                    p,
                    format!(
                        "{} {}",
                        b.packet.evidence.len(),
                        self.tr("źródeł w briefingu", "sources in briefing")
                    ),
                );
                ui.add_space(14.);
                if button(ui, p, self.tr("Otwórz briefing", "Open briefing"), false).clicked() {
                    self.selected_briefing = Some(b.result_id);
                    self.selected_citation = None;
                    self.navigate_to(2);
                }
            } else {
                ui.label(
                    RichText::new(self.tr("Miejsce na jasny obraz", "Room for a clearer picture"))
                        .size(21.),
                );
                meta(
                    ui,
                    p,
                    self.tr(
                        "Zapytaj MAIA o dodane źródła. Zapisany briefing pojawi się tutaj.",
                        "Ask MAIA about your sources. Your saved briefing will appear here.",
                    ),
                );
            }
        });
    }
    fn sources_card(&mut self, ui: &mut egui::Ui, p: Palette) {
        MaiaCard::show(ui, p, 210., |ui| {
            section(ui, p, 3, self.text("sources"));
            if self.artifacts.is_empty() {
                meta(ui, p, self.text("empty"));
            }
            for a in self.artifacts.iter().take(3) {
                ui.horizontal(|ui| {
                    icon(ui, 3, p.muted, 18.);
                    ui.label(truncate(a.name().as_str(), 42));
                });
            }
            ui.add_space(16.);
            if button(ui, p, self.tr("+  Dodaj źródło", "+  Add source"), false).clicked() {
                self.show_import = true;
            }
        });
    }
    fn local_card(&self, ui: &mut egui::Ui, p: Palette) {
        MaiaCard::show(ui, p, 210., |ui| {
            section(ui, p, 4, self.text("local"));
            ui.label(
                RichText::new(self.tr("Bliżej Twojej pracy", "Closer to your work"))
                    .size(23.)
                    .strong()
                    .family(egui::FontFamily::Name("maia-semibold".into())),
            );
            ui.add_space(6.);
            // Previously hardcoded chip(ui, p, "Qwen", false) / chip(ui, p,
            // "Offline", true) - unconditional and never wired to
            // `local_available` (same defect as the sidebar status meta;
            // Architecture Desk - "GO - PROCEED AUTONOMOUSLY" Step 4).
            ui.horizontal_wrapped(|ui| {
                chip(ui, p, "Qwen", self.local_available);
                chip(
                    ui,
                    p,
                    if self.local_available {
                        self.tr("Online", "Online")
                    } else {
                        self.tr("Offline", "Offline")
                    },
                    self.local_available,
                );
            });
            ui.add_space(12.);
            meta(ui,p,self.tr("Briefingi oparte na Twoich źródłach. Cytowania prowadzą do zapisanych fragmentów.","Briefings grounded in your sources. Citations lead back to recorded excerpts."));
        });
    }
    fn workspace_view(&mut self, ui: &mut egui::Ui, p: Palette) {
        self.workspace_card(ui, p);
        ui.add_space(20.);
        self.sources_view(ui, p);
    }
    fn sources_view(&mut self, ui: &mut egui::Ui, p: Palette) {
        ui.horizontal(|ui| {
            section(
                ui,
                p,
                3,
                self.tr("Dokumenty w obszarze", "Workspace documents"),
            );
            if button(ui, p, self.tr("+  Dodaj źródło", "+  Add source"), true).clicked() {
                self.show_import = true;
            }
        });
        if self.artifacts.is_empty() {
            MaiaCard::show(ui, p, 130., |ui| {
                ui.label(self.text("empty"));
            });
        }
        for a in &self.artifacts {
            MaiaCard::show(ui, p, 92., |ui| {
                ui.horizontal(|ui| {
                    icon(ui, 3, p.accent, 28.);
                    ui.label(
                        RichText::new(a.name().as_str())
                            .size(18.)
                            .strong()
                            .family(egui::FontFamily::Name("maia-semibold".into())),
                    );
                });
                meta(ui, p, truncate(a.content().as_str(), 180));
                ui.horizontal(|ui| {
                    chip(ui, p, self.tr("Zapisana kopia", "Saved snapshot"), true);
                    meta(
                        ui,
                        p,
                        format!(
                            "{} {}",
                            a.content().as_str().lines().count(),
                            self.tr("wierszy", "lines")
                        ),
                    );
                });
            });
            ui.add_space(8.);
        }
    }
    fn history_view(&mut self, ui: &mut egui::Ui, p: Palette) {
        let records = self.briefings.clone();
        if records.is_empty() {
            MaiaCard::show(ui, p, 180., |ui| {
                section(
                    ui,
                    p,
                    2,
                    self.tr(
                        "Tutaj wrócisz do swoich wniosków",
                        "Your insights, ready to revisit",
                    ),
                );
                meta(
                    ui,
                    p,
                    self.tr(
                        "Zadaj pytanie w pasku Zapytaj MAIA, korzystając z dodanych źródeł.",
                        "Ask a question in Ask MAIA using your imported sources.",
                    ),
                );
            });
        }
        for b in records.iter() {
            let selected = self.selected_briefing.as_deref() == Some(b.result_id.as_str());
            MaiaCard::show(ui, p, 60., |ui| {
                ui.horizontal_wrapped(|ui| {
                    if button(ui, p, &b.packet.objective, selected).clicked() {
                        self.selected_briefing = Some(b.result_id.clone());
                        self.selected_citation = None;
                    }
                    meta(ui, p, date(b.created_at.as_str()));
                });
                if selected {
                    ui.add_space(16.);
                    ui.horizontal(|ui| {
                        chip(
                            ui,
                            p,
                            self.tr("Oparte na źródłach", "Evidence grounded"),
                            true,
                        );
                        meta(
                            ui,
                            p,
                            format!(
                                "Qwen · {} {}",
                                b.packet.evidence.len(),
                                self.tr("źródeł", "sources")
                            ),
                        );
                    });
                    ui.add_space(18.);
                    for claim in &b.result.claims {
                        let label = match claim.class {
                            maia_briefing::ClaimClass::SupportedByEvidence => {
                                self.tr("ZE ŹRÓDEŁ", "FROM SOURCES")
                            }
                            maia_briefing::ClaimClass::Inference => self.tr("WNIOSEK", "INFERENCE"),
                            _ => self.tr("DO WYJAŚNIENIA", "UNKNOWN"),
                        };
                        meta(ui, p, label);
                        ui.label(RichText::new(&claim.text).size(16.));
                        ui.horizontal_wrapped(|ui| {
                            for c in &claim.citations {
                                if button(
                                    ui,
                                    p,
                                    &format!(
                                        "{}  {}",
                                        self.tr("Źródło", "Source"),
                                        c.trim_start_matches('e')
                                    ),
                                    false,
                                )
                                .clicked()
                                {
                                    self.selected_citation = Some(c.clone());
                                    self.context_tab = 1;
                                }
                            }
                        });
                        ui.add_space(20.);
                    }
                }
            });
            ui.add_space(10.);
        }
    }
    fn context(&mut self, ui: &mut egui::Ui, p: Palette) {
        ui.add_space(15.);
        ui.horizontal(|ui| {
            icon(ui, 4, p.accent, 24.);
            ui.label(
                RichText::new(self.tr("Kontekst", "Context"))
                    .size(21.)
                    .strong()
                    .family(egui::FontFamily::Name("maia-semibold".into())),
            );
        });
        ui.add_space(20.);
        ui.horizontal(|ui| {
            for (i, label) in [
                self.tr("Źródła", "Sources"),
                self.tr("Cytowania", "Citations"),
                self.tr("Szczegóły", "Details"),
            ]
            .iter()
            .enumerate()
            {
                if ui
                    .selectable_label(self.context_tab == i, RichText::new(*label).size(13.))
                    .clicked()
                {
                    self.context_tab = i;
                }
            }
        });
        ui.separator();
        ui.add_space(16.);
        egui::ScrollArea::vertical()
            .id_salt("context_scroll")
            .show(ui, |ui| {
                let briefing = self
                    .briefings
                    .iter()
                    .find(|b| Some(b.result_id.as_str()) == self.selected_briefing.as_deref())
                    .cloned();
                if self.context_tab == 2 {
                    section(ui, p, 6, self.tr("O tym obszarze", "About this workspace"));
                    meta(
                        ui,
                        p,
                        self.tr(
                            "Dokumenty i historia są zapisywane lokalnie.",
                            "Documents and history are stored locally.",
                        ),
                    );
                    ui.add_space(14.);
                    ui.collapsing(self.tr("Diagnostyka", "Diagnostics"), |ui| {
                        // self.status is written only by reload() ("Workspace
                        // ready · N snapshots · N briefings") and
                        // import_evidence() — perform_briefing() no longer
                        // writes it (Batch 1 review #1/#10), so this label is
                        // unaffected and keeps showing workspace status, not a
                        // stale failure message. Surfacing technical_reason
                        // for the last failed attempt here is Batch 3 scope
                        // (Details tab wiring), not yet done.
                        ui.label(&self.status);
                        ui.label(&self.database_path);
                        if let Some(b) = &briefing {
                            ui.label(&b.result_hash);
                            ui.label(&b.packet_hash);
                        }
                    });
                } else if self.context_tab == 1 {
                    if let Some(b) = briefing {
                        for e in &b.packet.evidence {
                            if self
                                .selected_citation
                                .as_deref()
                                .is_some_and(|s| s != e.citation_id)
                            {
                                continue;
                            }
                            MaiaCard::show(ui, p, 100., |ui| {
                                section(
                                    ui,
                                    p,
                                    3,
                                    &format!(
                                        "{} {}",
                                        self.tr("Źródło", "Source"),
                                        e.citation_id.trim_start_matches('e')
                                    ),
                                );
                                let name = self
                                    .artifacts
                                    .iter()
                                    .find(|a| a.id().as_str() == e.artifact_id)
                                    .map(|a| a.name().as_str())
                                    .unwrap_or(self.tr("Zapisane źródło", "Recorded source"));
                                ui.label(
                                    RichText::new(name)
                                        .strong()
                                        .family(egui::FontFamily::Name("maia-semibold".into()))
                                        .size(16.),
                                );
                                meta(
                                    ui,
                                    p,
                                    format!(
                                        "{} 1–{}",
                                        self.tr("Wiersze", "Lines"),
                                        e.excerpt.lines().count()
                                    ),
                                );
                                ui.add_space(10.);
                                egui::ScrollArea::vertical()
                                    .id_salt((&b.result_id, &e.citation_id))
                                    .max_height(380.)
                                    .show(ui, |ui| {
                                        ui.label(&e.excerpt);
                                    });
                                ui.add_space(12.);
                                chip(
                                    ui,
                                    p,
                                    self.tr("Niezmienny fragment", "Immutable excerpt"),
                                    true,
                                );
                                meta(
                                    ui,
                                    p,
                                    self.tr(
                                        "Z lokalnego źródła · kopia z chwili briefingu",
                                        "From a local source · snapshot at briefing time",
                                    ),
                                );
                            });
                            ui.add_space(12.);
                        }
                    } else {
                        self.context_empty(ui, p);
                    }
                } else if self.artifacts.is_empty() {
                    self.context_empty(ui, p);
                } else {
                    meta(ui, p, self.tr("W TWOIM OBSZARZE", "IN YOUR WORKSPACE"));
                    ui.add_space(10.);
                    for a in self.artifacts.iter().take(8) {
                        MaiaCard::show(ui, p, 65., |ui| {
                            icon(ui, 3, p.accent, 22.);
                            ui.label(
                                RichText::new(a.name().as_str())
                                    .strong()
                                    .family(egui::FontFamily::Name("maia-semibold".into())),
                            );
                            meta(
                                ui,
                                p,
                                format!(
                                    "{} {}",
                                    a.content().as_str().lines().count(),
                                    self.tr(
                                        "wierszy · zapisane lokalnie",
                                        "lines · stored locally"
                                    )
                                ),
                            );
                        });
                        ui.add_space(10.);
                    }
                    ui.add_space(16.);
                    meta(
                        ui,
                        p,
                        self.tr(
                            "Wybierz cytowanie w briefingu, aby zobaczyć dokładny fragment źródła.",
                            "Select a citation in a briefing to see the exact source excerpt.",
                        ),
                    );
                }
            });
    }
    fn context_empty(&self, ui: &mut egui::Ui, p: Palette) {
        MaiaCard::show(ui, p, 210., |ui| {
            ui.add_space(12.);
            icon(ui, 4, p.accent, 36.);
            ui.add_space(16.);
            ui.label(
                RichText::new(self.tr("Miejsce na kontekst", "A place for context"))
                    .size(20.)
                    .strong()
                    .family(egui::FontFamily::Name("maia-semibold".into())),
            );
            meta(
                ui,
                p,
                self.tr(
                    "Źródła i cytowania pojawią się tutaj, gdy zaczniesz pracować z dokumentami.",
                    "Sources and citations appear here as you work with documents.",
                ),
            );
        });
    }
}

#[cfg(test)]
mod design_tests {
    // Placed inside the design module (rather than main.rs's own test
    // module) specifically so these tests can call the private
    // `failure_copy()` — Batch 2's remaining Desk-required tests (#10/#11/
    // #12: Timeout and MalformedResponse map to their specified product
    // copy; raw Rust Debug text never appears in normal user-facing copy).
    use super::*;

    fn app(language: &str) -> MaiaDesktop {
        MaiaDesktop {
            language: language.into(),
            ..Default::default()
        }
    }

    #[test]
    fn timeout_maps_to_timeout_specific_copy_in_both_languages() {
        let en = app("en").failure_copy(Some(BriefingFailureKind::Timeout));
        assert!(en.to_lowercase().contains("time"));
        let pl = app("pl").failure_copy(Some(BriefingFailureKind::Timeout));
        assert!(pl.contains("czas") || pl.contains("odpowie"));
        assert_ne!(
            en,
            app("en").failure_copy(Some(BriefingFailureKind::MalformedResponse)),
            "Timeout and MalformedResponse must not share identical copy — \
             both are 'non-success' but users need to know the difference"
        );
    }

    #[test]
    fn malformed_response_maps_to_validation_failure_copy_not_timeout_copy() {
        let en = app("en").failure_copy(Some(BriefingFailureKind::MalformedResponse));
        assert!(en.to_lowercase().contains("valid"));
        assert!(!en.to_lowercase().contains("time out") && !en.to_lowercase().contains("busy"));
    }

    #[test]
    fn raw_debug_text_never_appears_in_failure_copy_for_any_kind() {
        // Every kind that can reach the UI must render through localized,
        // hand-written copy — never format!("{error:?}") or an internal
        // enum/variant name (Architecture Desk Batch 1 review #6).
        let banned = [
            "Provider(",
            "Timeout)",
            "MalformedResponse",
            "LocalProviderFailure",
            "BriefingError",
            "Debug",
            "{:?}",
        ];
        let kinds = [
            None,
            Some(BriefingFailureKind::Timeout),
            Some(BriefingFailureKind::MalformedResponse),
            Some(BriefingFailureKind::Unavailable),
            Some(BriefingFailureKind::ModelUnavailable),
            Some(BriefingFailureKind::ProviderError),
            Some(BriefingFailureKind::EmptyPrompt),
            Some(BriefingFailureKind::NoSources),
            Some(BriefingFailureKind::InvalidWorkspace),
            Some(BriefingFailureKind::InvalidRuntimeConfig),
            Some(BriefingFailureKind::ExecutionAuthorityRejected),
            Some(BriefingFailureKind::ClockError),
            Some(BriefingFailureKind::PersistenceError),
            Some(BriefingFailureKind::Other),
        ];
        for language in ["en", "pl"] {
            for kind in kinds {
                let copy = app(language).failure_copy(kind);
                for needle in banned {
                    assert!(
                        !copy.contains(needle),
                        "failure_copy({kind:?}) in {language} leaked raw Debug-ish \
                         text {needle:?}: {copy:?}"
                    );
                }
            }
        }
    }
}
