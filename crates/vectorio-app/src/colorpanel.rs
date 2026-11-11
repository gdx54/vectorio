//! Selector de color del sistema (NSColorPanel de macOS). Se abre con un color y la app
//! lo va leyendo en cada fotograma mientras siga visible, así el cambio se ve en vivo.

#[cfg(target_os = "macos")]
mod imp {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSColor, NSColorPanel, NSColorSpace};

    pub fn open(c: [u8; 3]) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let panel = NSColorPanel::sharedColorPanel(mtm);
        panel.setShowsAlpha(false);
        let f = |v: u8| v as f64 / 255.0;
        let color = NSColor::colorWithSRGBRed_green_blue_alpha(f(c[0]), f(c[1]), f(c[2]), 1.0);
        panel.setColor(&color);
        panel.makeKeyAndOrderFront(None);
    }

    /// Color actual del panel, o `None` si está cerrado.
    pub fn read() -> Option<[u8; 3]> {
        let mtm = MainThreadMarker::new()?;
        if !NSColorPanel::sharedColorPanelExists(mtm) {
            return None;
        }
        let panel = NSColorPanel::sharedColorPanel(mtm);
        if !panel.isVisible() {
            return None;
        }
        let c = panel.color().colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())?;
        let u = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        Some([u(c.redComponent()), u(c.greenComponent()), u(c.blueComponent())])
    }

    pub fn close() {
        if let Some(mtm) = MainThreadMarker::new() {
            if NSColorPanel::sharedColorPanelExists(mtm) {
                NSColorPanel::sharedColorPanel(mtm).orderOut(None);
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn open(_: [u8; 3]) {}
    pub fn read() -> Option<[u8; 3]> {
        None
    }
    pub fn close() {}
}

pub use imp::{close, open, read};
