mod app;
mod config;
mod file_tree;
mod layout;
mod scan;
mod workspace;

use app::MarkSpaceApp;
use eframe::egui;

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([640.0, 400.0])
            .with_title("MarkSpace"),
        ..Default::default()
    };

    eframe::run_native(
        "MarkSpace",
        native_options,
        Box::new(|cc| Ok(Box::new(MarkSpaceApp::new(cc)))),
    )
}
