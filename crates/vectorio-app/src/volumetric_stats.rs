//! Resumen y estimación de salida en segundo plano, con la configuración real del documento.
use super::*;
#[derive(Clone, PartialEq)]
struct Key {
    version: u64,
    passes: Vec<(u64, usize, bool, Vec<usize>)>,
    paper: Paper,
    pens: Vec<Pen>,
    plot: vectorio_core::PlotSettings,
    output: vectorio_core::output::OutputSettings,
    dynamics: vectorio_core::output::Dynamics,
}
#[derive(Default)]
pub(super) struct Estimate {
    cancel: Arc<std::sync::atomic::AtomicBool>,
    key: Option<Key>,
    seconds: Option<f64>,
    /// La última petición ya respondió (con o sin tiempo).
    done: bool,
    version: u64,
    worker: Option<(
        mpsc::Sender<(u64, Key, Drawing, Arc<std::sync::atomic::AtomicBool>)>,
        mpsc::Receiver<(u64, Option<f64>)>,
        Arc<AtomicU64>,
    )>,
}
impl Estimate {
    fn request(&mut self, ctx: &egui::Context, key: Key, drawing: Drawing) {
        if self.key.as_ref() == Some(&key) {
            return;
        }
        if self.worker.is_none() {
            let (tx, rx) =
                mpsc::channel::<(u64, Key, Drawing, Arc<std::sync::atomic::AtomicBool>)>();
            let (out, input) = mpsc::channel();
            let latest = Arc::new(AtomicU64::new(0));
            let active = latest.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                while let Ok(mut job) = rx.recv() {
                    for next in rx.try_iter() {
                        job = next;
                    }
                    let (v, key, drawing, abort) = job;
                    let cancel = || active.load(Ordering::Relaxed) != v;
                    if cancel() {
                        continue;
                    }
                    let seconds = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let doc = drawing.document(key.paper.clone(), key.pens.clone(), []);
                        let layers = doc.layers();
                        vectorio_core::output::prepare_cancellable(
                            &layers,
                            &key.paper,
                            &key.pens,
                            &key.plot,
                            &key.output,
                            &key.dynamics,
                            &abort,
                        )
                        .ok()
                        .map(|p| p.stats.total_seconds)
                    }))
                    .ok()
                    .flatten();
                    if !cancel() {
                        let _ = out.send((v, seconds));
                        ctx.request_repaint();
                    }
                }
            });
            self.worker = Some((tx, input, latest));
        }
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.version = self.version.wrapping_add(1);
        self.seconds = None;
        self.done = false;
        let (tx, _, latest) = self.worker.as_ref().unwrap();
        latest.store(self.version, Ordering::Relaxed);
        if tx
            .send((self.version, key.clone(), drawing, self.cancel.clone()))
            .is_ok()
        {
            self.key = Some(key);
        }
    }
    fn poll(&mut self) {
        if let Some((_, rx, _)) = &self.worker {
            for (v, seconds) in rx.try_iter() {
                if v == self.version {
                    self.seconds = seconds;
                    self.done = true;
                }
            }
        }
    }
}
impl Drop for Estimate {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some((_, _, latest)) = &self.worker {
            latest.fetch_add(1, Ordering::Relaxed);
        }
    }
}
pub(super) fn footer(
    ui: &mut egui::Ui,
    state: &mut VolumetricUi,
    app: &VectorioApp,
) -> (bool, bool) {
    let current = state.current(&app.doc.paper);
    state.estimate.poll();
    let key = Key {
        version: state.version,
        passes: state
            .passes
            .iter()
            .map(|p| {
                (
                    p.id,
                    p.pass.pen,
                    p.pass.enabled,
                    p.pass
                        .reading
                        .as_ref()
                        .map_or_else(Vec::new, |m| m.pens.clone()),
                )
            })
            .collect(),
        paper: app.doc.paper.clone(),
        pens: app.doc.pens.clone(),
        plot: app.doc.plot.clone(),
        output: app.doc.output.clone(),
        dynamics: app.prefs.machine.dynamics(),
    };
    if current && state.estimate.key.as_ref() != Some(&key) {
        if let Some(drawing) = state.materialized(&app.doc.pens) {
            state.estimate.request(ui.ctx(), key, drawing);
        }
    }
    let n: usize = state
        .passes
        .iter()
        .filter(|p| p.pass.enabled)
        .filter_map(|p| state.part(p.id))
        .map(|p| p.lines.len())
        .sum();
    let inks: std::collections::BTreeSet<_> = state
        .passes
        .iter()
        .filter(|p| p.pass.enabled && state.part(p.id).is_some_and(|p| !p.lines.is_empty()))
        .flat_map(|p| {
            state
                .part(p.id)
                .into_iter()
                .flat_map(|part| part.colors(&p.pass, &app.doc.pens))
                .filter(|(_, lines)| !lines.is_empty())
                .map(|(pen, _)| pen)
        })
        .collect();
    let mut place = false;
    let mut export = false;
    let pal = theme::current(ui.ctx());
    theme::glass_panel_with(
        ui,
        &pal,
        theme::CONTROL_RADIUS + 6.,
        egui::Margin::symmetric(12., 7.),
        theme::Material::Panel,
        |ui| {
            ui.horizontal(|ui| {
                let milliseconds = state
                    .drawing
                    .as_ref()
                    .map_or(0., |d| (d.seconds * 1000.).round());
                // Sin tiempo fiable no se muestra un «~— min»: se dice que se calcula o se omite.
                let time = match state.estimate.seconds {
                    Some(s) => Some(format!("≈ {}", crate::fmt::duration(s))),
                    None if current && n > 0 && !state.estimate.done => {
                        Some(tr!("calculando tiempo…"))
                    }
                    None => None,
                };
                let mut summary = tr!(
                    "%{strokes} trazos · %{inks} tintas",
                    strokes = n,
                    inks = inks.len()
                );
                for part in time.into_iter().chain([format!("{milliseconds} ms")]) {
                    summary.push_str(" · ");
                    summary.push_str(&part);
                }
                ui.weak(summary)
                .on_hover_text(tr!("Estimación con los ajustes de Salida."));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_enabled_ui(current && n > 0 && !app.doc.pens.is_empty(), |ui| {
                        place = theme::accent_button(
                            ui,
                            &if state.editing() {
                                tr!("Actualizar en el lienzo")
                            } else {
                                tr!("Añadir al lienzo")
                            },
                        )
                        .clicked();
                        export = ui.button(tr!("Exportar SVG del modelo")).clicked();
                    });
                });
            });
        },
    );
    (place, export)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn estimate_uses_the_actual_output_speed_and_keeps_only_the_latest_result() {
        let ctx = egui::Context::default();
        let app = VectorioApp::blank(&ctx);
        let mut state = VolumetricUi::default();
        state.mesh = Arc::new(Primitive::Cube.mesh());
        super::super::tests::prepare(&mut state, &app.doc.paper);
        let drawing = state.materialized(&app.doc.pens).unwrap();
        let key = Key {
            version: 0,
            passes: vec![],
            paper: app.doc.paper.clone(),
            pens: app.doc.pens.clone(),
            plot: app.doc.plot.clone(),
            output: app.doc.output.clone(),
            dynamics: app.prefs.machine.dynamics(),
        };
        let mut estimate = Estimate::default();
        let wait = |estimate: &mut Estimate| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while estimate.seconds.is_none() && std::time::Instant::now() < deadline {
                estimate.poll();
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            estimate.seconds.expect("la estimación debe terminar")
        };
        estimate.request(&ctx, key.clone(), drawing.clone());
        let fast = wait(&mut estimate);
        let mut slow = key.clone();
        slow.plot.draw_speed /= 4.;
        estimate.request(&ctx, slow, drawing.clone());
        let slow = wait(&mut estimate);
        assert!(slow > fast);
        let mut obsolete = key.clone();
        obsolete.plot.draw_speed = 1.;
        estimate.request(&ctx, obsolete, drawing.clone());
        estimate.request(&ctx, key, drawing);
        assert_eq!(wait(&mut estimate), fast);
    }
}
