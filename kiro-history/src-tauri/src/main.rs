#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Set environment variables BEFORE GTK/WebKit initialization.
    // These must be set in main() — setting them inside run() is too late
    // because GTK reads GDK_BACKEND at gtk_init() time.
    #[cfg(target_os = "linux")]
    {
        // Force Wayland backend when WSLg or Wayland is available.
        // This enables Windows IME to work via WSLg's text-input protocol.
        if std::env::var("GDK_BACKEND").is_err() && std::env::var("WAYLAND_DISPLAY").is_ok() {
            // Safety: single-threaded at this point
            unsafe {
                std::env::set_var("GDK_BACKEND", "wayland");
            }
        }

        // Disable GPU compositing to avoid MESA/ZINK errors on WSL2.
        if std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").is_err() {
            unsafe {
                std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
            }
        }
        if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
            unsafe {
                std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
            }
        }
    }

    hi_kiro_lib::run()
}
