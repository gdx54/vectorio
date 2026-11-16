use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use super::{GenOutput, InkChoice, Params};
use crate::{Document, Geometry, Item, Pen, Placement, BBox};

/// Receta editable, identidades de partes y tintas recordadas incluso si desaparecen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeneratedGroup {
    pub id: u64, pub name: String, pub generator: String, pub params: Params, pub seed: u64, pub area: BBox,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parts: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inks: BTreeMap<String, Option<usize>>,
    #[serde(default)] pub placement: Placement,
    /// Juego de tintas elegido en Crear; `None` reparte los rotuladores del documento.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ink_choice: Option<InkChoice>,
}
impl GeneratedGroup {
    pub fn new(name: String, generator: String, params: Params, seed: u64, area: BBox) -> Self {
        Self { id: 0, name, generator, params, seed, area, parts: BTreeMap::new(), inks: BTreeMap::new(),
            placement: Placement { pos: area.min, ..Default::default() }, ink_choice: None }
    }
}
impl Document {
    pub fn generated_group(&self, id: u64) -> Option<&GeneratedGroup> { self.generated.iter().find(|g| g.id == id) }
    /// Una parte es un objeto. Sus coordenadas son locales al origen del área original;
    /// Placement conserva la transformación del usuario, nunca se aplica su inversa a
    /// los trazos nuevos. Así regenerar no deshace un movimiento o escalado anterior.
    pub fn put_generated(&mut self, mut group: GeneratedGroup, output: GenOutput, replacing: Option<u64>) -> Vec<u64> {
        let old = replacing.and_then(|id| self.generated_group(id)).cloned();
        if let Some(old) = &old {
            group.id = old.id;
            group.parts = old.parts.clone(); group.inks = old.inks.clone();
            group.placement = self.group_items(old.id).first().and_then(|id| self.get(*id)).map_or(old.placement, |it| it.place);
            for (name, id) in &old.parts { if let Some(it) = self.get(*id) { group.inks.insert(name.clone(), it.stroke); } }
        } else { group.id = self.next_id; self.next_id += 1; }
        let names: Vec<_> = output.parts.iter().map(|p| p.name.clone()).collect();
        let removed: Vec<_> = group.parts.iter().filter(|(name, _)| !names.contains(name)).map(|(_, id)| *id).collect();
        self.items.retain(|it| !removed.contains(&it.id));
        group.parts.retain(|name, _| names.contains(name));
        let mut ids = Vec::new();
        for (i, part) in output.parts.into_iter().enumerate() {
            let local = part.lines.into_iter().map(|line| line.into_iter().map(|q| q - group.area.min).collect()).collect();
            let geometry = Geometry::Path { lines: local, closed: false };
            let id = if let Some(id) = group.parts.get(&part.name).copied().filter(|id| self.get(*id).is_some()) {
                if let Some(it) = self.get_mut(id) { it.geometry = geometry; }
                id
            } else {
                let mut it = Item::new(format!("{} · {}", group.name, part.name), geometry, group.placement.pos);
                it.place = group.placement; it.group = Some(group.id);
                let suggested = part.pen_order.unwrap_or(i);
                let pen = if self.pens.is_empty() { None } else { Some(if suggested == usize::MAX { self.pens.len() - 1 } else { suggested % self.pens.len() }) };
                it.stroke = group.inks.get(&part.name).copied().unwrap_or(pen).filter(|p| *p < self.pens.len());
                let id = self.add(it); group.parts.insert(part.name.clone(), id); id
            };
            if let Some(it) = self.get(id) { group.inks.insert(part.name, it.stroke); }
            ids.push(id);
        }
        if let Some(index) = self.generated.iter().position(|g| g.id == group.id) { self.generated[index] = group; }
        else { self.generated.push(group); }
        ids
    }
    /// Como `put_generated`, con las tintas de un juego (`parte → rotulador`).
    /// Reutiliza el rotulador del documento con el mismo color o añade uno nuevo.
    /// Al regenerar con el mismo juego se respetan las tintas que el usuario haya
    /// cambiado; un juego distinto, o una parte nueva, recibe la tinta del juego.
    pub fn put_generated_with_inks(&mut self, group: GeneratedGroup, output: GenOutput, replacing: Option<u64>,
        inks: &BTreeMap<String, Pen>) -> Vec<u64> {
        let old = replacing.and_then(|id| self.generated_group(id));
        let keep = old.filter(|g| g.ink_choice == group.ink_choice).map(|g| g.inks.keys().cloned().collect::<Vec<_>>());
        let names: Vec<_> = output.parts.iter().map(|p| p.name.clone()).collect();
        let ids = self.put_generated(group, output, replacing);
        let Some(&first) = ids.first() else { return ids };
        let Some(group_id) = self.get(first).and_then(|it| it.group) else { return ids };
        for (name, id) in names.iter().zip(&ids) {
            if keep.as_ref().is_some_and(|known| known.contains(name)) { continue; }
            let Some(pen) = inks.get(name) else { continue };
            let index = self.pen_index_for(pen);
            if let Some(it) = self.get_mut(*id) { it.stroke = Some(index); }
            if let Some(g) = self.generated.iter_mut().find(|g| g.id == group_id) { g.inks.insert(name.clone(), Some(index)); }
        }
        ids
    }
    fn pen_index_for(&mut self, pen: &Pen) -> usize {
        if let Some(i) = self.pens.iter().position(|p| p.color == pen.color) { return i; }
        self.pens.push(pen.clone()); self.pens.len() - 1
    }
}
