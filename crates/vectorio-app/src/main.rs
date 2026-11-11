//! Vectorio · aplicación de escritorio.

mod app;
mod canvas;
mod colorpanel;
mod commands;
mod inspector;
mod theme;
mod vectorize_ui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Vectorio")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([980.0, 620.0])
            // Barra de herramientas unificada: el contenido llega bajo los botones de la ventana.
            .with_fullsize_content_view(true)
            .with_title_shown(false)
            .with_titlebar_shown(false)
            .with_drag_and_drop(true),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native("Vectorio", options, Box::new(|cc| Ok(Box::new(app::VectorioApp::new(cc)))))
}
