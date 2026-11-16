use super::*;
/// Réouvre la recette du groupe sans toucher au document jusqu'à « Actualiser ».
pub(super) fn reopen(app: &mut VectorioApp, group: u64) {
    let Some(volume) = app.doc.volume(group).cloned() else {
        return;
    };
    let Some(recipe) = app.doc.volume_recipe(group) else {
        return;
    };
    let mut state = std::mem::take(&mut app.volumetric_ui);
    state.worker = None;
    state.import_epoch.fetch_add(1, Ordering::Relaxed);
    state.loading = None;
    state.mesh = Arc::new(recipe.mesh);
    state.report = state
        .mesh
        .as_ref()
        .clone()
        .validated()
        .map(|(_, r)| r)
        .unwrap_or_default();
    state.primitive = None;
    state.settings = recipe.settings;
    state.replace_passes(recipe.passes, volume.name, false);
    state.editing = Some(group);
    state.drawing = None;
    state.requested = None;
    state.ready = false;
    state.error = None;
    state.load_error = None;
    app.volumetric_ui = state;
    app.mode = ViewMode::Volumetric;
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reopening_a_volume_keeps_the_document_and_recovers_recipe_inks() {
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        let mut state = VolumetricUi::default();
        state.mesh = Arc::new(Primitive::Cube.mesh());
        state.settings.projection.perspective = true;
        state.settings.sheet = SheetViews::Turntable;
        state.settings.view_count = 4;
        super::super::tests::prepare(&mut state, &app.doc.paper);
        let volume = VolumeObject {
            id: 0,
            name: "Objeto editable".into(),
            recipe: state.recipe(&app),
            placement: Default::default(),
            bindings: vec![],
        };
        let ids = app
            .doc
            .put_volume(volume, &state.materialized(&app.doc.pens).unwrap(), None)
            .unwrap();
        let group = app.doc.get(ids[0]).unwrap().group.unwrap();
        app.doc.get_mut(ids[0]).unwrap().stroke = Some(3);
        let before = app.doc.clone();
        reopen(&mut app, group);
        assert_eq!(app.doc, before);
        assert_eq!(app.mode, ViewMode::Volumetric);
        assert_eq!(app.volumetric_ui.editing, Some(group));
        assert!(app.volumetric_ui.settings.projection.perspective);
        assert_eq!(app.volumetric_ui.settings.sheet, SheetViews::Turntable);
        assert_eq!(app.volumetric_ui.passes[0].pass.pen, 3);
    }
    #[test]
    fn opening_a_generated_scene_uses_a_worker_and_keeps_existing_objects() {
        let ctx = egui::Context::default();
        let mut app = VectorioApp::blank(&ctx);
        let mut state = VolumetricUi::default();
        let before = app.doc.clone();
        let mut params = vectorio_core::generate::Params::default();
        params.set("rows", vectorio_core::generate::ParamValue::Number(1.));
        params.set("cols", vectorio_core::generate::ParamValue::Number(1.));
        state.open_scene(
            &ctx,
            "blocks".into(),
            params,
            vectorio_core::generate::GenContext {
                area: vectorio_core::BBox {
                    min: p(15., 15.),
                    max: p(180., 120.),
                },
                seed: 3,
                pen_mm: 0.4,
                detail: vectorio_core::generate::Detail::Final,
            },
            app.doc.paper.clone(),
            app.doc.pens.clone(),
        );
        let until = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while state.loading.is_some() && std::time::Instant::now() < until {
            state.poll(&mut app);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(state.loading.is_none());
        assert!(state.load_error.is_none(), "{:?}", state.load_error);
        assert_eq!(app.doc, before);
        assert_eq!(state.mesh.name, "Manzanas");
        assert!(state.mesh.triangles.len() > 12);
        assert!(state.editing.is_none());
    }
    #[test]
    fn sheet_and_projection_controls_preserve_recipe_and_local_undo() {
        let mut state = VolumetricUi::default();
        state.checkpoint();
        state.settings.sheet = SheetViews::Turntable;
        state.settings.view_count = 8;
        state.settings.projection.perspective = true;
        let expected = state.snapshot();
        state.undo_stack(false);
        assert_eq!(state.settings.sheet, SheetViews::Single);
        state.undo_stack(true);
        assert_eq!(state.settings, expected.settings);
        assert!(state.settings.validate().is_ok());
    }
}
