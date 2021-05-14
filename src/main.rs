mod gemini;
mod ui;

fn main() {
    let app = ui::App::default();
    let options = eframe::epi::NativeOptions::default();
    eframe::run_native(Box::new(app), options);
}
