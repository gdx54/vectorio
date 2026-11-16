# Vectorio

- Rust desktop application for pen plotters. The core is in `crates/vectorio-core`; the egui interface is in `crates/vectorio-app`.
- Preserve Spanish in the interface and the separation between objects/layers and inks.
- Add `/opt/homebrew/opt/rustup/bin` to PATH if `cargo` is unavailable.
- After code changes, run the appropriate checks and update the macOS installation with `python3 scripts/install-macos.py`. The user-authorized installation destination is `/Applications/Vectorio.app`.
- Do not close or restart windows containing the user's work. An already open app continues running its current version until the next launch.
- The installed app runs its binary directly, without a launcher that reads the source project, so macOS does not request access to Documents on every launch. The installer preserves the previous version if the build fails.
- For visual checks, prefer the installation in Applications. Do not create additional copies of the app unless necessary to preserve an open user session.
- The chosen logo is **E · Relief (Relieve)**, with the **A02 · Soft lemon / indigo** palette: background `#F6F095` and a contour-line V in `#35216C`. Branding assets are in `assets/branding`; the macOS icon is in `packaging/macos/Vectorio.icns`, generated with `cargo run -p vectorio-core --example icono_macos --release` (an 824 px plate centered within 1024 px).
