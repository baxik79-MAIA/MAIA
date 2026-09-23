//! Opt-in native framebuffer capture for visual review; never supplies synthetic records.
use super::*;
impl MaiaDesktop {
    pub(super) fn capture_review(&mut self, ctx: &egui::Context) {
        let Ok(path) = std::env::var("MAIA_CAPTURE_PATH") else {
            return;
        };
        self.capture_frame += 1;
        if self.capture_frame == 1 {
            if let Ok(language) = std::env::var("MAIA_REVIEW_LANGUAGE") {
                self.language = language;
            }
            if let Ok(theme) = std::env::var("MAIA_REVIEW_THEME") {
                self.dark_theme = theme == "dark";
            }
            if let Ok(page) = std::env::var("MAIA_REVIEW_PAGE") {
                self.page = page.parse::<usize>().unwrap_or(0).min(3);
            }
            if let Ok(import) = std::env::var("MAIA_REVIEW_IMPORT") {
                self.evidence_path = import;
                self.import_evidence();
            }
            if let Ok(prompt) = std::env::var("MAIA_REVIEW_PROMPT") {
                self.prompt = prompt;
                self.ask_maia();
            }
            if self.page == 2 {
                self.selected_briefing = self.briefings.first().map(|b| b.result_id.clone());
            }
            if std::env::var_os("MAIA_REVIEW_CITATION").is_some() {
                self.context_tab = 1;
                self.selected_citation = self
                    .briefings
                    .first()
                    .and_then(|b| b.packet.evidence.first())
                    .map(|e| e.citation_id.clone());
            }
        }
        if self.briefing_job.is_some() {
            self.capture_frame = 2;
        }
        if self.capture_frame == 12 || ctx.input(|i| i.key_pressed(egui::Key::F12)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event {
                let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                match image::save_buffer(
                    &path,
                    &bytes,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                ) {
                    Ok(()) => {
                        let _ = fs::write(format!("{path}.status.txt"), &self.status);
                        if std::env::var_os("MAIA_CAPTURE_KEEP_OPEN").is_none() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                    Err(e) => {
                        let _ = fs::write(format!("{path}.error.txt"), e.to_string());
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        }
        ctx.request_repaint_after(Duration::from_millis(80));
    }
}
