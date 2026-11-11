//! Lanzador de la instalación de desarrollo de Vectorio para macOS.
//! Actualiza al abrir si cambió el proyecto; conserva el último binario si falla.

use std::{env, fs::{self, OpenOptions}, path::PathBuf, process::Command};
use std::os::unix::process::CommandExt;

fn main() {
    let executable = env::current_exe().expect("No se pudo localizar Vectorio");
    let bundle = executable.parent().and_then(|p| p.parent()).and_then(|p| p.parent())
        .expect("Vectorio debe estar dentro de su aplicación");
    let source = bundle.join("Contents/Resources/project-path.txt");
    let project = fs::read_to_string(source).ok().map(|s| PathBuf::from(s.trim()));
    if let Some(project) = project {
        let installer = project.join("scripts/install-macos.py");
        if installer.is_file() {
            let logs = env::var_os("HOME").map(PathBuf::from)
                .unwrap_or_else(env::temp_dir).join("Library/Logs/Vectorio");
            let _ = fs::create_dir_all(&logs);
            let mut update = Command::new("/usr/bin/python3");
            update.arg(installer).arg("--if-needed").arg("--destination").arg(bundle);
            if let Ok(log) = OpenOptions::new().create(true).append(true).open(logs.join("updates.log")) {
                if let Ok(stderr) = log.try_clone() { update.stderr(stderr); }
                update.stdout(log);
            }
            // Si el código no compila, la instalación anterior sigue siendo utilizable.
            let _ = update.status();
        }
    }
    let error = Command::new(bundle.join("Contents/MacOS/vectorio"))
        .args(env::args_os().skip(1)).exec();
    eprintln!("No se pudo abrir Vectorio: {error}");
    std::process::exit(1);
}
