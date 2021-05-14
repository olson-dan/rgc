use crate::gemini::request;
use eframe::{egui, epi};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};

#[derive(PartialEq, Eq)]
enum AppState {
    Browsing,
    Loading,
    NewContent(String, String),
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
    state: Arc<Mutex<AppState>>,
}

impl App {
    fn go_to_url(&mut self, url: String) {
        let previous_url = self.url_stack.last().cloned().unwrap_or_default();
        let mut state = self.state.lock().unwrap();
        *state.deref_mut() = AppState::Loading;
        let task_state = self.state.clone();
        async_std::task::spawn(async move {
            let (url, contents) = request(&previous_url, &url).await;
            let mut state = task_state.lock().unwrap();
            *state.deref_mut() = AppState::NewContent(url, contents);
        });
    }
}

impl epi::App for App {
    fn update(&mut self, ctx: &egui::CtxRef, _frame: &mut epi::Frame<'_>) {
        let mut loading = false;
        let mut goto_url = None;
        {
            let mut state = self.state.lock().unwrap();
            if let AppState::NewContent(url, content) = state.deref() {
                self.url = url.clone();
                self.url_stack.push(self.url.clone());
                self.contents = content.clone();
                *state.deref_mut() = AppState::Browsing;
            } else if AppState::Loading == *state.deref() {
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
                let mut preformatted = false;
                for line in self.contents.lines() {
                    if line.starts_with("#") {
                        ui.heading(line.trim_start_matches('#'));
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
                        ui.code(line);
                    } else {
                        ui.label(line);
                    }
                }
            });
        });

        if let Some(url) = goto_url {
            self.go_to_url(url);
        }

        {
            let mut output = ctx.output();
            // Show progress cursor if loading a page.
            if loading {
                output.cursor_icon = egui::CursorIcon::Progress;
            }
            // Kill default hyperlink behavior.
            output.open_url = None;
        }
    }

    fn name(&self) -> &str {
        "RGC Gemini Browser"
    }
}
