//! Un único catálogo para generadores y sus variantes de escena.
use super::*;

pub(super) struct Choice {
    pub generator: &'static str,
    pub variant: Option<(&'static str, &'static str)>,
    pub label: String,
    pub category: String,
}

pub(super) fn variant_parameter(generator: &str) -> Option<&'static str> {
    match generator {
        "domes" => Some("world"),
        "harbour" => Some("port"),
        "megastructure" => Some("structure"),
        "festival_city" => Some("event"),
        "spiro" => Some("kind"),
        _ => None,
    }
}

impl Choice {
    pub fn key(&self) -> String {
        self.variant.map_or_else(
            || self.generator.into(),
            |(id, value)| format!("{}/{id}/{value}", self.generator),
        )
    }
    pub fn params(&self) -> Params {
        let mut params = Params::default();
        if let Some((id, value)) = self.variant {
            params.set(id, ParamValue::Text(value.into()));
        }
        params
    }
    fn matches(&self, settings: &CreatePrefs) -> bool {
        if self.generator != settings.generator {
            return false;
        }
        self.variant.is_none_or(|(id, value)| {
            let generator = generate::find(self.generator).unwrap();
            settings
                .params
                .get(self.generator)
                .cloned()
                .unwrap_or_default()
                .normalized(&generator.params())
                .choice(id)
                == value
        })
    }
}

pub(super) fn choices() -> Vec<Choice> {
    let mut choices = vec![];
    for generator in generate::all() {
        let category = crate::i18n::tr_key(match generator.id() {
            "white_village" | "blocks" | "megastructure" => tm!("Arquitectura"),
            "festival_city" => tm!("Ciudades"),
            "harbour" => tm!("Puertos"),
            "domes" => tm!("Mundos"),
            _ => generator.category(),
        });
        let variant = variant_parameter(generator.id()).and_then(|id| {
            generator.params().into_iter().find_map(|spec| match spec {
                ParamSpec::Choice {
                    id: parameter,
                    options,
                    ..
                } if id == parameter => Some((id, options)),
                _ => None,
            })
        });
        if let Some((id, options)) = variant {
            for (value, label) in options {
                choices.push(Choice {
                    generator: generator.id(),
                    variant: Some((id, *value)),
                    label: format!(
                        "{} · {}",
                        crate::i18n::tr_key(generator.name()),
                        crate::i18n::tr_key(label)
                    ),
                    category: category.clone(),
                });
            }
        } else {
            choices.push(Choice {
                generator: generator.id(),
                variant: None,
                label: crate::i18n::tr_key(generator.name()),
                category,
            });
        }
    }
    // Agrupa categorías sin depender del orden de registro de los generadores.
    let mut order = Vec::new();
    for choice in &choices {
        if !order.contains(&choice.category) {
            order.push(choice.category.clone());
        }
    }
    choices.sort_by_key(|choice| order.iter().position(|c| c == &choice.category).unwrap());
    choices
}

impl CreateUi {
    fn choose_scene(&mut self, choice: &Choice) {
        if self.settings.generator != choice.generator {
            self.cancel();
        }
        self.settings.generator = choice.generator.into();
        if let Some((id, value)) = choice.variant {
            self.settings
                .params
                .entry(choice.generator.into())
                .or_default()
                .set(id, ParamValue::Text(value.into()));
        }
        self.invalidate();
    }
}

pub(super) fn show(ui: &mut egui::Ui, state: &mut CreateUi) -> bool {
    let choices = choices();
    let active = choices
        .iter()
        .position(|choice| choice.matches(&state.settings));
    ui.horizontal(|ui| {
        ui.weak(tr!("Escena"));
        if let Some(id) = variant_parameter(&state.settings.generator) {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut locked = state.settings.locks.contains(id);
                lock_button(ui, &mut locked, &tr!("Escena"));
                if locked {
                    state.settings.locks.insert(id.into());
                } else {
                    state.settings.locks.remove(id);
                }
            });
        }
    });
    let entries = choices
        .iter()
        .map(|c| crate::catalog::Entry {
            label: c.label.clone(),
            category: c.category.clone(),
        })
        .collect::<Vec<_>>();
    let label = active.map_or_else(|| tr!("Elegir escena"), |i| choices[i].label.clone());
    let selected = crate::catalog::selector(
        ui,
        egui::Id::new("create-generator"),
        &label,
        active,
        &entries,
        ui.available_width(),
        |i, ui| {
            state.thumbnails.request(ui.ctx(), &choices[i]);
            if let Some(output) = state.thumbnails.outputs.get(&choices[i].key()) {
                thumbnail(ui, output, generate::find(choices[i].generator).unwrap());
            } else {
                ui.weak(tr!("Preparando miniatura…"));
            }
        },
    );
    if let Some(i) = selected {
        if active != Some(i) {
            state.choose_scene(&choices[i]);
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_generator_and_primary_variant_has_one_catalog_entry_and_a_distinct_thumbnail() {
        let choices = choices();
        let keys = choices.iter().map(Choice::key).collect::<BTreeSet<_>>();
        assert_eq!(keys.len(), choices.len());
        for generator in generate::all() {
            let expected = variant_parameter(generator.id()).map_or(1, |id| {
                generator
                    .params()
                    .into_iter()
                    .find_map(|p| match p {
                        ParamSpec::Choice {
                            id: key, options, ..
                        } if key == id => Some(options.len()),
                        _ => None,
                    })
                    .unwrap()
            });
            assert_eq!(
                choices
                    .iter()
                    .filter(|c| c.generator == generator.id())
                    .count(),
                expected
            );
            let settings = CreatePrefs {
                generator: generator.id().into(),
                ..Default::default()
            };
            assert_eq!(
                choices.iter().filter(|c| c.matches(&settings)).count(),
                1,
                "receta antigua sin parámetros"
            );
            for choice in choices.iter().filter(|c| c.generator == generator.id()) {
                let settings = CreatePrefs {
                    generator: generator.id().into(),
                    params: BTreeMap::from([(generator.id().into(), choice.params())]),
                    ..Default::default()
                };
                assert!(choice.matches(&settings));
            }
        }
    }
    #[test]
    fn switching_variant_preserves_other_parameters_seed_and_the_group_being_edited() {
        let mut state = CreateUi::default();
        state.settings.generator = "domes".into();
        state.settings.seed = 729;
        state.seed_text = "729".into();
        state
            .settings
            .params
            .entry("domes".into())
            .or_default()
            .set("domes", ParamValue::Number(9.));
        state.editing = Some(42);
        state.settings.locks.insert("domes".into());
        let choice = choices()
            .into_iter()
            .find(|c| c.variant == Some(("world", "space")))
            .unwrap();
        state.choose_scene(&choice);
        assert_eq!(state.settings.params["domes"].choice("world"), "space");
        assert_eq!(state.settings.params["domes"].f64("domes"), 9.);
        assert_eq!(state.settings.seed, 729);
        assert_eq!(state.seed_text, "729");
        assert_eq!(state.editing, Some(42));
        assert!(state.settings.locks.contains("domes"));
    }
}
