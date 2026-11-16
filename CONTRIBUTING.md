# Contributing to Vectorio

Vectorio started as a tool for my own plotter drawings. I'd be happy to have other people help shape it: fixes, new generators or drawing effects, usability improvements, documentation, and examples are all welcome.

## Getting started

Use stable Rust and run:

```sh
cargo test --workspace --locked
cargo run --release -p vectorio-app
```

On macOS, install or update the app with:

```sh
python3 scripts/install-macos.py
```

The installer targets `/Applications/Vectorio.app`. It preserves the previous version if compilation fails. Keep any windows containing work open; they continue running their existing version until the next launch.

## Windows and Linux

Builds and ports for other operating systems are welcome. The implementation uses Rust, egui, and wgpu, but only macOS has been tested so far. Native menus, file dialogs, graphics backend features, user-data paths, shortcuts, and packaging may need platform-specific work.

An AI assistant can help investigate build errors and platform APIs. Please test the result on the actual operating system and include your OS version, Rust version, build command, required dependencies, and test results with a contribution. Build instructions for untested platforms should be clearly marked as provisional.

## Working on a change

- Keep objects/layers independent from inks.
- Preserve the Spanish interface and add matching English translations in `crates/vectorio-app/locales/app.yml`.
- Keep SVG output vector-based and measured in millimeters; respect pen widths and paper clipping.
- Keep expensive calculations off the interface thread and preserve cancellation behavior.
- Explain the problem, resulting behavior, and relevant validation in your pull request. Run checks appropriate to the change.
- Preserve historical SVG equivalence where a change is intended to leave existing algorithms unchanged.

For the project layout and current limitations, see [README.md](README.md) and the [feature guide](LEEME.md).
