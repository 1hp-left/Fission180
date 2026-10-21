#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod engine_examples;
mod icons;
mod view;
mod worker;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Fission 180")
            .with_inner_size([1536., 864.])
            .with_min_inner_size([1040., 650.])
            .with_decorations(false)
            .with_maximized(true),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "Fission 180",
        options,
        Box::new(|cc| Ok(Box::new(app::FusionApp::new(cc)))),
    )
}
