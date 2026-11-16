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
    use std::sync::{OnceLock, atomic::{AtomicBool, Ordering}};

    static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);
    static CONTEXT: OnceLock<egui::Context> = OnceLock::new();

    // Dock y cierre de sesión pasan por el mismo control de cambios que el menú.
    unsafe extern "C-unwind" fn should_terminate(_this: &AnyObject, _cmd: Sel, _app: &AnyObject) -> isize {
        QUIT_REQUESTED.store(true, Ordering::Relaxed);
        show_window();
        if let Some(ctx) = CONTEXT.get() { ctx.request_repaint(); }
        0 // NSTerminateCancel: el siguiente fotograma resuelve guardar/descartar/cancelar.
    }

    pub fn take_quit_request() -> bool { QUIT_REQUESTED.swap(false, Ordering::Relaxed) }

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
    pub fn install_reopen_handler(ctx: &egui::Context) {
        let _ = CONTEXT.set(ctx.clone());
        let Some(class) = AnyClass::get(c"WinitApplicationDelegate") else { return };
        let Ok(types) = CString::new(format!("{}@:@{}", Bool::ENCODING, Bool::ENCODING)) else { return };
        let handler: unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject, Bool) -> Bool = should_handle_reopen;
        // SAFETY: la firma coincide con el selector y la codificación de tipos declarada;
        // class_addMethod no sustituye un método si el delegado ya lo implementa.
        unsafe {
            let imp: objc2::runtime::Imp = std::mem::transmute(handler);
            ffi::class_addMethod(class as *const AnyClass as *mut AnyClass, sel!(applicationShouldHandleReopen:hasVisibleWindows:), imp, types.as_ptr());
            let terminate_types = CString::new(format!("{}@:@", isize::ENCODING)).expect("codificación estática sin ceros");
            let terminate_handler: unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject) -> isize = should_terminate;
            let terminate_imp: objc2::runtime::Imp = std::mem::transmute(terminate_handler);
            ffi::class_addMethod(class as *const AnyClass as *mut AnyClass, sel!(applicationShouldTerminate:), terminate_imp, terminate_types.as_ptr());
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
pub use imp::{install_reopen_handler, show_window, take_quit_request};

#[cfg(not(target_os = "macos"))]
pub fn install_reopen_handler(_ctx: &egui::Context) {}

#[cfg(not(target_os = "macos"))]
pub fn take_quit_request() -> bool { false }

/// Deja que eframe termine el bucle después del fotograma y libere sus recursos.
/// NSApplication::terminate desde App::update reentra en el manejador de winit
/// y provoca un pánico en applicationWillTerminate (no admite desenrollado).
pub fn quit(ctx: &egui::Context) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    ctx.request_repaint();
}

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
