//! Comportamiento de ventana propio de macOS: ⌘W y el botón rojo ocultan la ventana sin
//! salir de la app ni perder el documento; un clic en el Dock la vuelve a mostrar.
//! ⌘Q («Salir de Vectorio») sigue cerrando el programa.

#[cfg(target_os = "macos")]
mod imp {
    use objc2::encode::Encode;
    use objc2::runtime::{AnyClass, AnyObject, Bool, Sel};
    use objc2::{ffi, sel, MainThreadMarker};
    use objc2_app_kit::NSApplication;
    use std::ffi::CString;

    /// Muestra las ventanas principales que estén ocultas. Solo en el hilo principal.
    pub fn show_window() {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let app = NSApplication::sharedApplication(mtm);
        for window in app.windows().iter() {
            if window.canBecomeMainWindow() && !window.isVisible() && !window.isMiniaturized() {
                window.makeKeyAndOrderFront(None);
            }
        }
    }

    unsafe extern "C-unwind" fn should_handle_reopen(_this: &AnyObject, _cmd: Sel, _app: &AnyObject, has_visible_windows: Bool) -> Bool {
        if !has_visible_windows.as_bool() {
            show_window();
        }
        // AppKit completa el resto (por ejemplo, restaurar una ventana minimizada).
        Bool::YES
    }

    /// Añade `applicationShouldHandleReopen:hasVisibleWindows:` al delegado de winit.
    /// Debe llamarse con el bucle de eventos ya creado (desde la creación de la app).
    pub fn install_reopen_handler() {
        let Some(class) = AnyClass::get(c"WinitApplicationDelegate") else { return };
        let Ok(types) = CString::new(format!("{}@:@{}", Bool::ENCODING, Bool::ENCODING)) else { return };
        let handler: unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject, Bool) -> Bool = should_handle_reopen;
        // SAFETY: la firma coincide con el selector y la codificación de tipos declarada;
        // class_addMethod no sustituye un método si el delegado ya lo implementa.
        unsafe {
            let imp: objc2::runtime::Imp = std::mem::transmute(handler);
            ffi::class_addMethod(class as *const AnyClass as *mut AnyClass, sel!(applicationShouldHandleReopen:hasVisibleWindows:), imp, types.as_ptr());
        }
        // AppKit puede recordar qué métodos tenía el delegado al asignarlo: se reasigna
        // para que tenga en cuenta el que acabamos de añadir.
        if let Some(mtm) = MainThreadMarker::new() {
            let app = NSApplication::sharedApplication(mtm);
            if let Some(delegate) = app.delegate() {
                app.setDelegate(Some(&delegate));
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub use imp::{install_reopen_handler, show_window};

#[cfg(not(target_os = "macos"))]
pub fn install_reopen_handler() {}

#[cfg(not(target_os = "macos"))]
pub fn show_window() {}

/// ⌘W o botón rojo: en macOS se oculta la ventana; en otros sistemas se cierra.
pub fn close_window(ctx: &egui::Context) {
    if cfg!(target_os = "macos") {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
    } else {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}
