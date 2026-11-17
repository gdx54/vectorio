//! Selector compartido por Crear y Volumen: búsqueda, categorías y miniatura al pasar.
use egui::{Id, Ui};
#[derive(Clone)]
pub(crate) struct Entry {
    pub label: String,
    pub category: String,
}
fn folded(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            _ => c,
        })
        .collect()
}
fn matches(entry: &Entry, query: &str) -> bool {
    let text = folded(&format!("{} {}", entry.category, entry.label));
    folded(query)
        .split_whitespace()
        .all(|word| text.contains(word))
}
pub(crate) fn selector(
    ui: &mut Ui,
    id: Id,
    label: &str,
    selected: Option<usize>,
    entries: &[Entry],
    width: f32,
    mut preview: impl FnMut(usize, &mut Ui),
) -> Option<usize> {
    let query_id = id.with("query");
    let mut query = ui
        .ctx()
        .data(|d| d.get_temp::<String>(query_id))
        .unwrap_or_default();
    let mut choice = None;
    let height = (ui.ctx().screen_rect().height() * 0.55).min(400.);
    let opened_id = id.with("was-open");
    let was_open = ui.ctx().data(|d| d.get_temp::<bool>(opened_id)).unwrap_or(false);
    let menu = egui::ComboBox::from_id_salt(id)
        .icon(crate::theme::combo_icon)
        .selected_text(label)
        .width(width)
        .height(height)
        .show_ui(ui, |ui| {
            ui.set_min_width(260.);
            ui.set_max_width(330.);
            let search = ui.add(
                egui::TextEdit::singleline(&mut query)
                    .hint_text(tr!("Buscar por nombre o categoría…"))
                    .desired_width(f32::INFINITY),
            );
            if !was_open { search.request_focus(); }
            let mut found = false;
            egui::ScrollArea::vertical()
                .id_salt(id.with("list"))
                .max_height((height - 44.).max(100.))
                .show(ui, |ui| {
                    let mut category = "";
                    for (index, entry) in entries.iter().enumerate() {
                        if !matches(entry, &query) {
                            continue;
                        }
                        found = true;
                        if category != entry.category {
                            if !category.is_empty() {
                                ui.add_space(8.);
                            }
                            category = &entry.category;
                            ui.label(egui::RichText::new(category).small().strong());
                        }
                        let response = ui.selectable_label(selected == Some(index), &entry.label);
                        if !was_open && query.is_empty() && selected == Some(index) {
                            response.scroll_to_me(Some(egui::Align::Center));
                        }
                        if response.clicked() {
                            choice = Some(index);
                            ui.close_menu();
                        }
                        response.on_hover_ui(|ui| preview(index, ui));
                    }
                });
            if !found {
                ui.weak(tr!("No hay resultados"));
            }
        });
    ui.ctx().data_mut(|d| d.insert_temp(opened_id, menu.inner.is_some()));
    if choice.is_some() {
        query.clear();
    }
    ui.ctx().data_mut(|d| d.insert_temp(query_id, query));
    choice
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_matches_words_in_name_and_category_without_accents() {
        let entry = Entry {
            label: "Mapa térmico".into(),
            category: "Mapas de lectura".into(),
        };
        assert!(matches(&entry, "termico lectura"));
        assert!(!matches(&entry, "termico ondas"));
    }
    #[test]
    fn searchable_catalog_selects_a_filtered_entry_with_the_pointer() {
        let ctx = egui::Context::default();
        let entries = vec![
            Entry {
                label: "Ondas".into(),
                category: "Líneas".into(),
            },
            Entry {
                label: "Mapa térmico".into(),
                category: "Mapas".into(),
            },
        ];
        let id = Id::new("catalog-test");
        let mut chosen = None;
        let mut frame = |time, events| {
            ctx.run(
                egui::RawInput {
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        chosen = selector(ui, id, "Ondas", Some(0), &entries, 240., |_, _| {});
                    });
                },
            )
        };
        let out = frame(0., vec![]);
        let point = |out: &egui::FullOutput, label: &str| {
            out.shapes
                .iter()
                .find_map(|s| {
                    if let egui::Shape::Text(t) = &s.shape {
                        (t.galley.text() == label).then_some(t.pos + t.galley.size() * 0.5)
                    } else {
                        None
                    }
                })
                .unwrap()
        };
        let pos = point(&out, "Ondas");
        let _ = frame(
            0.1,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        let _ = frame(
            0.2,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        // A persisted query also exercises reopening without a horizontal gallery.
        ctx.data_mut(|d| d.insert_temp(id.with("query"), "termico".to_string()));
        let out = frame(0.3, vec![]);
        let pos = point(&out, "Mapa térmico");
        let _ = frame(
            0.4,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        let _ = frame(
            0.5,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
        assert_eq!(chosen, Some(1));
    }
}
