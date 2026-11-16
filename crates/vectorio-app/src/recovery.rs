//! Copias atómicas en segundo plano, separadas del archivo guardado por el usuario.
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};
use vectorio_core::Document;
#[derive(Serialize, Deserialize)]
pub(crate) struct Snapshot {
    pub doc: Document,
    pub file: Option<PathBuf>,
}
enum Job {
    Write(Snapshot),
    Clear,
    Finish,
}
#[derive(Default)]
pub(crate) struct Recovery {
    tx: Option<mpsc::Sender<Job>>,
    thread: Option<std::thread::JoinHandle<()>>,
    revision: Option<u64>,
    last: Option<Instant>,
    present: bool,
}
impl Recovery {
    pub fn start(dir: PathBuf) -> Self {
        if std::fs::create_dir_all(&dir).is_err() {
            return Self::default();
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = dir.join(format!("{}-{stamp}.vectorio", std::process::id()));
        let (tx, rx) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            while let Ok(mut job) = rx.recv() {
                for next in rx.try_iter() {
                    job = next;
                }
                match job {
                    Job::Write(snapshot) => {
                        if let Ok(bytes) = serde_json::to_vec(&snapshot) {
                            let _ = crate::files::atomic_write(&path, &bytes);
                        }
                    }
                    Job::Clear => {
                        let _ = std::fs::remove_file(&path);
                    }
                    Job::Finish => {
                        let _ = std::fs::remove_file(&path);
                        break;
                    }
                }
            }
        });
        Self {
            tx: Some(tx),
            thread: Some(thread),
            ..Default::default()
        }
    }
    pub fn update(
        &mut self,
        ctx: &egui::Context,
        doc: &Document,
        revision: u64,
        file: &Option<PathBuf>,
        dirty: bool,
    ) {
        let Some(tx) = &self.tx else {
            return;
        };
        if !dirty {
            if self.present {
                let _ = tx.send(Job::Clear);
                self.present = false;
            }
            self.revision = None;
            return;
        }
        if self.revision == Some(revision) {
            return;
        }
        if self
            .last
            .is_none_or(|t| t.elapsed() >= Duration::from_secs(2))
        {
            let _ = tx.send(Job::Write(Snapshot {
                doc: doc.clone(),
                file: file.clone(),
            }));
            self.revision = Some(revision);
            self.last = Some(Instant::now());
            self.present = true;
        }
        ctx.request_repaint_after(Duration::from_secs(2));
    }
    pub fn finish(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Job::Finish);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
pub(crate) fn candidates(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut paths = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            if p.extension().is_none_or(|e| e != "vectorio") {
                return false;
            }
            let pid = p
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| s.split('-').next())
                .and_then(|s| s.parse::<u32>().ok());
            if pid == Some(std::process::id()) {
                return false;
            }
            #[cfg(unix)]
            if let Some(pid) = pid {
                if std::process::Command::new("/bin/kill")
                    .args(["-0", &pid.to_string()])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .is_ok_and(|s| s.success())
                {
                    return false;
                }
            }
            true
        })
        .collect::<Vec<_>>();
    paths.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    paths.reverse();
    paths
}
pub(crate) fn read(path: &std::path::Path) -> Result<Snapshot, String> {
    let text = crate::files::read_project(path).map_err(|e| e.to_string())?;
    let mut snapshot: Snapshot = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    snapshot.doc = Document::from_json(&snapshot.doc.to_json())?;
    Ok(snapshot)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn automatic_snapshot_keeps_editable_3d_data_and_clean_exit_removes_only_its_copy() {
        use vectorio_core::volumetric::*;
        let dir = tempfile::tempdir().unwrap();
        let other = dir.path().join("other.txt");
        std::fs::write(&other, "keep").unwrap();
        let mut doc = Document::default();
        let mut recipe = Recipe::new(Primitive::Cube.mesh(), Settings::default());
        recipe.inks = doc.pens.clone();
        recipe.settings.projection.perspective = true;
        recipe.settings.sheet = SheetViews::Orthographic;
        let drawing = Engine::default().draw_recipe(&recipe, &|| false).unwrap();
        let ids = doc
            .put_volume(
                VolumeObject {
                    id: 0,
                    name: "Recuperable".into(),
                    recipe,
                    placement: Default::default(),
                    bindings: vec![],
                },
                &drawing,
                None,
            )
            .unwrap();
        assert!(!ids.is_empty());
        let ctx = egui::Context::default();
        let mut recovery = Recovery::start(dir.path().into());
        recovery.update(&ctx, &doc, 3, &None, true);
        let deadline = Instant::now() + Duration::from_secs(5);
        let path = loop {
            if let Some(p) = std::fs::read_dir(dir.path())
                .unwrap()
                .filter_map(Result::ok)
                .map(|e| e.path())
                .find(|p| p.extension().is_some_and(|e| e == "vectorio"))
            {
                break p;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        };
        let restored = read(&path).unwrap();
        assert_eq!(restored.doc, doc);
        assert!(
            restored.doc.volumes[0]
                .recipe
                .settings
                .projection
                .perspective
        );
        recovery.finish();
        assert!(!path.exists());
        assert_eq!(std::fs::read_to_string(other).unwrap(), "keep");
    }
    #[test]
    fn recovery_loader_rejects_invalid_projects_without_removing_them() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("99999999-0.vectorio");
        let mut doc = Document::default();
        doc.items.push(vectorio_core::Item::new(
            "Inválido",
            vectorio_core::Geometry::Path {
                lines: vec![],
                closed: false,
            },
            vectorio_core::p(0., 0.),
        ));
        std::fs::write(
            &path,
            serde_json::to_vec(&Snapshot { doc, file: None }).unwrap(),
        )
        .unwrap();
        assert!(read(&path).is_err());
        assert!(path.exists());
        assert!(candidates(dir.path()).contains(&path));
    }
}
