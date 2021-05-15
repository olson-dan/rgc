use crate::gemini::request;
use eframe::{egui, epi};
use egui::output::OpenUrl;
use std::sync::{Arc, Mutex};

#[derive(PartialEq, Eq)]
enum AppState {
    Browsing,
    Loading,
    NewContent(String, String, String),
}

impl Default for AppState {
    fn default() -> Self {
        AppState::Browsing
    }
}
#[derive(Default)]
pub struct App {
    url_stack: Vec<String>,
    url: String,
    contents: String,
    mimetype: String,
    state: Arc<Mutex<AppState>>,
}

impl App {
    fn go_to_url(&mut self, url: String) {
        let previous_url = self.url_stack.last().cloned().unwrap_or_default();
        let state = &mut *self.state.lock().unwrap();
        *state = AppState::Loading;
        let task_state = self.state.clone();
        async_std::task::spawn(async move {
            let (url, mimetype, contents) = request(&previous_url, &url).await;
            let state = &mut *task_state.lock().unwrap();
            *state = AppState::NewContent(url, mimetype, contents);
        });
    }
}

impl epi::App for App {
    fn update(&mut self, ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>) {
        let mut loading = false;
        let mut goto_url = None;
        {
            let state = &mut *self.state.lock().unwrap();
            if let AppState::NewContent(url, mimetype, content) = state {
                self.url = url.clone();
                self.url_stack.push(self.url.clone());
                self.contents = content.clone();
                self.mimetype = mimetype.clone();
                *state = AppState::Browsing;
            } else if AppState::Loading == *state {
                loading = true;
            }
        }

        egui::TopPanel::top("address_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.group(|ui| {
                    ui.set_enabled(self.url_stack.len() > 1);
                    if ui.button("<-").clicked() {
                        self.url_stack.pop();
                        if let Some(url) = self.url_stack.last() {
                            goto_url = Some(url.to_string())
                        }
                    }
                });
                let response = ui.add_sized(
                    ui.available_size(),
                    egui::TextEdit::singleline(&mut self.url).hint_text("Enter a URL"),
                );

                if response.lost_focus() && ui.input().key_pressed(egui::Key::Enter) {
                    self.go_to_url(self.url.clone());
                }
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::auto_sized().show(ui, |ui| {
                if self.mimetype == "text/gemini" {
                    let mut preformatted = false;
                    for line in self.contents.lines() {
                        if line.starts_with("#") {
                            ui.heading(line.trim_start_matches('#').trim_start());
                        } else if line.starts_with("=>") {
                            let mut splits = line.splitn(3, |c: char| c.is_whitespace());
                            let _ = splits.next();
                            let url = splits.next().unwrap_or_default().trim();
                            let label = splits.next().unwrap_or(url).trim();
                            if ui.hyperlink_to(label, url).clicked() {
                                goto_url = Some(url.to_string());
                            }
                        } else if line.starts_with("```") {
                            preformatted = !preformatted;
                        } else if preformatted {
                            ui.monospace(line);
                        } else {
                            ui.label(line);
                        }
                    }
                } else {
                    ui.monospace(&self.contents);
                }
            });
        });

        {
            let mut output = ctx.output();
            // Show progress cursor if loading a page.
            if loading {
                output.cursor_icon = egui::CursorIcon::Progress;
            }
            // Kill default hyperlink behavior.
            output.open_url = None;
        }

        if let Some(url) = goto_url {
            if url.starts_with("http") {
                ctx.output().open_url = Some(OpenUrl::new_tab(url));
            } else {
                self.go_to_url(url);
            }
        }
    }

    fn name(&self) -> &str {
        "RGC Gemini Browser"
    }
}
