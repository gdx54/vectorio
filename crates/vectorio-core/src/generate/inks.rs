//! Juegos de tintas por generador: cuántos rotuladores usa una escena y de qué color.
//!
//! Cada parte tiene un papel («línea», «trama», «vegetación»…). Un juego ordena sus
//! tintas por prioridad: con n rotuladores se usan las n primeras, y los papeles sin
//! rotulador pasan a su papel de reserva. Así 1, 2, 3 o 4 rotuladores dan siempre
//! un dibujo coherente, y la separación entre partes (objetos) y tintas se conserva.
use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use crate::doc::Pen;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ink { pub role: &'static str, pub name: &'static str, pub color: [u8; 3], pub width_mm: f64 }
#[derive(Debug)]
pub struct Palette {
    pub id: &'static str,
    pub name: &'static str,
    pub inks: &'static [Ink],
    /// Reservas propias del juego, antes de las de la receta.
    pub fallback: &'static [(&'static str, &'static str)],
}
#[derive(Debug)]
pub struct InkPlan {
    /// Parte → papel. Una parte no listada toma el primer papel, o se reparte en
    /// ciclo entre las tintas activas si `cycle` (capas del espirógrafo).
    pub parts: &'static [(&'static str, &'static str)],
    /// Papel → papel de reserva cuando no hay rotulador para él.
    pub fallback: &'static [(&'static str, &'static str)],
    pub palettes: &'static [Palette],
    pub default_count: usize,
    pub cycle: bool,
}
/// Elección guardada en la receta y en las preferencias de Crear.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InkChoice { pub palette: String, pub count: usize }

impl InkPlan {
    pub fn palette(&self, id: &str) -> Option<&Palette> { self.palettes.iter().find(|p| p.id == id) }
    pub fn default_choice(&self) -> InkChoice {
        let palette = &self.palettes[0];
        InkChoice { palette: palette.id.into(), count: self.default_count.clamp(1, palette.inks.len()) }
    }
    /// Juego conocido y número de rotuladores dentro de lo que ofrece.
    pub fn normalized(&self, choice: &InkChoice) -> InkChoice {
        let Some(palette) = self.palette(&choice.palette) else { return self.default_choice() };
        InkChoice { palette: palette.id.into(), count: choice.count.clamp(1, palette.inks.len()) }
    }
    pub fn active(&self, choice: &InkChoice) -> &'static [Ink] {
        let choice = self.normalized(choice);
        let inks = self.palette(&choice.palette).map_or(self.palettes[0].inks, |p| p.inks);
        &inks[..choice.count]
    }
    /// Tinta de la parte `name`, que ocupa la posición `index` en la salida.
    pub fn ink_for(&self, choice: &InkChoice, index: usize, name: &str) -> Ink {
        let choice = self.normalized(choice);
        let palette = self.palette(&choice.palette).unwrap();
        let active = self.active(&choice);
        let listed = self.parts.iter().find(|(part, _)| *part == name).map(|(_, role)| *role);
        let Some(mut role) = listed else {
            return if self.cycle { active[index % active.len()] } else { active[0] };
        };
        // La cadena de reservas es corta; el límite evita un ciclo mal declarado.
        for _ in 0..8 {
            if let Some(ink) = active.iter().find(|ink| ink.role == role) { return *ink; }
            let Some((_, next)) = palette.fallback.iter().chain(self.fallback.iter()).find(|(from, _)| *from == role) else { break };
            role = next;
        }
        active[0]
    }
    pub fn assign<'a>(&self, choice: &InkChoice, names: impl IntoIterator<Item = &'a str>) -> BTreeMap<String, Pen> {
        names.into_iter().enumerate().map(|(i, name)| {
            let ink = self.ink_for(choice, i, name);
            (name.to_owned(), Pen { name: ink.name.into(), color: ink.color, width_mm: ink.width_mm, custom_name: true })
        }).collect()
    }
}

const fn ink(role: &'static str, name: &'static str, color: [u8; 3], width_mm: f64) -> Ink { Ink { role, name, color, width_mm } }

// Escenas. Juegos de cartel: línea, sombra arrojada, tejas, vegetación y terreno.
// Juegos de grabado: línea, vegetación, trama de luz y terreno; tejas y sombras
// comparten la trama. Cada papel sin rotulador pasa a su reserva.
const SCENE_PARTS: &[(&str, &str)] = &[
    ("contorno", "línea"), ("detalle", "línea"), ("tejados", "tejado"), ("acento", "tejado"),
    ("trama", "trama"), ("trama clara", "trama"), ("trama media", "trama"), ("trama oscura", "trama"), ("sombra", "sombra"),
    ("vegetación", "vegetación"), ("terreno", "terreno"), ("gente", "línea"),
];
const SCENE_FALLBACK: &[(&str, &str)] = &[
    ("tejado", "trama"), ("sombra", "trama"), ("terreno", "trama"), ("trama", "línea"), ("vegetación", "línea"),
];
const SCENE_PALETTES: &[Palette] = &[
    Palette { id: "poster", name: "Cartel", fallback: &[], inks: &[
        ink("línea", "Negro", [30, 30, 34], 0.35), ink("sombra", "Azul cobalto", [36, 74, 202], 0.3),
        ink("tejado", "Rojo teja", [222, 64, 46], 0.3), ink("vegetación", "Verde prado", [32, 146, 102], 0.4),
        ink("terreno", "Ocre", [196, 140, 44], 0.3)] },
    Palette { id: "lime_tile", name: "Cal y teja", fallback: &[], inks: &[
        ink("línea", "Añil", [43, 47, 107], 0.35), ink("sombra", "Azul cal", [90, 123, 181], 0.3),
        ink("tejado", "Teja", [186, 84, 52], 0.3), ink("vegetación", "Olivo", [94, 112, 52], 0.4),
        ink("terreno", "Arena", [190, 150, 90], 0.3)] },
    Palette { id: "lime_indigo", name: "Cal y añil", fallback: &[], inks: &[
        ink("línea", "Añil", [43, 47, 107], 0.4), ink("vegetación", "Olivo", [94, 112, 52], 0.5),
        ink("trama", "Azul cal", [90, 123, 181], 0.3), ink("terreno", "Ocre", [181, 128, 58], 0.3)] },
    Palette { id: "earth_olive", name: "Tierra y olivo", fallback: &[], inks: &[
        ink("línea", "Sepia", [59, 42, 32], 0.4), ink("vegetación", "Oliva oscuro", [86, 98, 40], 0.5),
        ink("trama", "Terracota", [192, 98, 59], 0.3), ink("terreno", "Siena", [140, 90, 43], 0.3)] },
    Palette { id: "india_ink", name: "Tinta china", fallback: &[], inks: &[
        ink("línea", "Negro fino", [28, 28, 32], 0.4), ink("vegetación", "Verde gris", [79, 91, 74], 0.5),
        ink("trama", "Gris medio", [122, 122, 128], 0.3), ink("terreno", "Gris cálido", [138, 127, 114], 0.3)] },
    Palette { id: "dusk", name: "Atardecer", fallback: &[], inks: &[
        ink("línea", "Berenjena", [58, 37, 80], 0.4), ink("vegetación", "Verde mar", [47, 127, 115], 0.5),
        ink("trama", "Rosa palo", [217, 102, 138], 0.3), ink("terreno", "Naranja tostado", [224, 138, 60], 0.3)] },
    Palette { id: "vectorio", name: "Vectorio", fallback: &[], inks: &[
        ink("línea", "Índigo Vectorio", [53, 33, 108], 0.4), ink("vegetación", "Verde hoja", [78, 139, 95], 0.5),
        ink("trama", "Lavanda", [131, 78, 202], 0.3), ink("terreno", "Mostaza", [184, 140, 30], 0.3)] },
    // Cuatro colores clásicos; el amarillo queda como quinto bolígrafo opcional.
    Palette { id: "ballpoint", name: "Bolígrafo", fallback: &[("terreno", "línea")], inks: &[
        ink("línea", "Negro", [28, 30, 33], 0.35),
        ink("trama", "Azul", [22, 58, 155], 0.35),
        ink("tejado", "Rojo", [198, 36, 48], 0.35),
        ink("vegetación", "Verde", [16, 118, 67], 0.35),
        ink("terreno", "Amarillo", [224, 180, 0], 0.35),
    ] },
];
pub static SCENES: InkPlan = InkPlan { parts: SCENE_PARTS, fallback: SCENE_FALLBACK, palettes: SCENE_PALETTES, default_count: 5, cycle: false };

// Escenas densas (ciudad en fiesta, cúpulas): de 5 a 7 rotuladores. Tras la
// línea van la gente y la vegetación, que son lo que da vida a la lámina; el agua,
// la trama, los rótulos y el terreno se añaden después. Sin rotulador, la gente y
// los rótulos vuelven a la línea, y el agua, las tejas y las sombras a la trama.
const DENSE_PARTS: &[(&str, &str)] = &[
    ("contorno", "línea"),
    ("detalle", "línea"),
    ("gente", "gente"),
    ("rótulos", "rótulos"),
    ("agua", "agua"),
    ("tejados", "tejado"),
    ("acento", "tejado"),
    ("trama", "trama"),
    ("trama clara", "trama"),
    ("trama media", "trama"),
    ("trama oscura", "trama"),
    ("sombra", "sombra"),
    ("vegetación", "vegetación"),
    ("terreno", "terreno"),
];
const DENSE_FALLBACK: &[(&str, &str)] = &[
    ("gente", "línea"),
    ("rótulos", "gente"),
    ("agua", "trama"),
    ("tejado", "gente"),
    ("sombra", "agua"),
    ("terreno", "trama"),
    ("trama", "línea"),
    ("vegetación", "línea"),
];
const DENSE_PALETTES: &[Palette] = &[
    Palette { id: "festival", name: "Fiesta", fallback: &[], inks: &[
        ink("línea", "Negro", [30, 30, 34], 0.35),
        ink("gente", "Rojo vivo", [214, 52, 60], 0.3),
        ink("vegetación", "Verde prado", [32, 146, 102], 0.4),
        ink("agua", "Azul cobalto", [36, 74, 202], 0.3),
        ink("trama", "Lavanda", [131, 78, 202], 0.3),
        ink("rótulos", "Magenta", [196, 48, 140], 0.3),
        ink("terreno", "Ocre", [196, 140, 44], 0.3)] },
    Palette { id: "abyss", name: "Abismo", fallback: &[], inks: &[
        ink("línea", "Azul noche", [27, 48, 92], 0.35),
        ink("gente", "Coral", [232, 110, 90], 0.3),
        ink("vegetación", "Verde mar", [47, 127, 115], 0.4),
        ink("agua", "Turquesa", [38, 160, 160], 0.3),
        ink("trama", "Azul mar", [46, 110, 170], 0.3),
        ink("rótulos", "Ámbar", [232, 168, 40], 0.3),
        ink("terreno", "Arena", [205, 168, 105], 0.3)] },
    Palette { id: "orbit", name: "Órbita", fallback: &[], inks: &[
        ink("línea", "Berenjena", [58, 37, 80], 0.35),
        ink("gente", "Naranja tostado", [224, 138, 60], 0.3),
        ink("vegetación", "Verde hoja", [78, 139, 95], 0.4),
        ink("agua", "Azul cal", [90, 123, 181], 0.3),
        ink("trama", "Rosa palo", [217, 102, 138], 0.3),
        ink("rótulos", "Mostaza", [184, 140, 30], 0.3),
        ink("terreno", "Gris cálido", [138, 127, 114], 0.3)] },
    Palette { id: "vectorio", name: "Vectorio", fallback: &[], inks: &[
        ink("línea", "Índigo Vectorio", [53, 33, 108], 0.35),
        ink("gente", "Rojo teja", [222, 64, 46], 0.3),
        ink("vegetación", "Verde hoja", [78, 139, 95], 0.4),
        ink("agua", "Azul cal", [90, 123, 181], 0.3),
        ink("trama", "Lavanda", [131, 78, 202], 0.3)] },
    // Cuatro colores clásicos; el amarillo queda como quinto bolígrafo opcional.
    Palette { id: "ballpoint", name: "Bolígrafo", fallback: &[("tejado", "gente"), ("rótulos", "línea"), ("terreno", "línea")], inks: &[
        ink("línea", "Negro", [28, 30, 33], 0.35),
        ink("trama", "Azul", [22, 58, 155], 0.35),
        ink("gente", "Rojo", [198, 36, 48], 0.35),
        ink("vegetación", "Verde", [16, 118, 67], 0.35),
        ink("terreno", "Amarillo", [224, 180, 0], 0.35),
    ] },
];
pub static DENSE: InkPlan = InkPlan { parts: DENSE_PARTS, fallback: DENSE_FALLBACK, palettes: DENSE_PALETTES, default_count: 5, cycle: false };

const DOME_PALETTES: &[Palette] = &[
    Palette { id: "abyss", name: "Abismo", inks: DENSE_PALETTES[1].inks, fallback: &[] },
    Palette { id: "orbit", name: "Órbita", inks: DENSE_PALETTES[2].inks, fallback: &[] },
    Palette { id: "ballpoint", name: "Bolígrafo", inks: DENSE_PALETTES[4].inks, fallback: DENSE_PALETTES[4].fallback },
];
pub static DOMES: InkPlan = InkPlan { parts: DENSE_PARTS, fallback: DENSE_FALLBACK, palettes: DOME_PALETTES, default_count: 5, cycle: false };

// El puerto prioriza agua, tejado y sombra; con cinco bolígrafos mantiene todos.
const HARBOUR_PALETTES:&[Palette]=&[
    Palette{id:"harbour",name:"Costa viva",fallback:&[],inks:&[
        ink("línea","Índigo",[40,38,102],0.35),ink("agua","Turquesa",[12,141,167],0.3),
        ink("tejado","Bermellón",[222,64,46],0.35),ink("sombra","Violeta",[113,61,188],0.3),
        ink("vegetación","Verde esmeralda",[20,139,94],0.4),ink("terreno","Ocre dorado",[189,130,27],0.3)]},
    Palette{id:"ballpoint",name:"Bolígrafo",inks:DENSE_PALETTES[4].inks,fallback:DENSE_PALETTES[4].fallback},
];
pub static HARBOUR:InkPlan=InkPlan{parts:DENSE_PARTS,fallback:DENSE_FALLBACK,palettes:HARBOUR_PALETTES,default_count:5,cycle:false};

// Espirógrafo: cada capa toma la siguiente tinta activa.
const SPIRO_PALETTES: &[Palette] = &[
    Palette { id: "vectorio", name: "Vectorio", fallback: &[], inks: &[
        ink("capa", "Índigo Vectorio", [53, 33, 108], 0.4), ink("capa", "Lavanda", [131, 78, 202], 0.4),
        ink("capa", "Mostaza", [184, 140, 30], 0.4), ink("capa", "Rosa palo", [217, 102, 138], 0.4),
        ink("capa", "Verde mar", [47, 127, 115], 0.4), ink("capa", "Naranja tostado", [224, 138, 60], 0.4)] },
    Palette { id: "ocean", name: "Océano", fallback: &[], inks: &[
        ink("capa", "Azul noche", [27, 48, 92], 0.4), ink("capa", "Azul mar", [46, 110, 170], 0.4),
        ink("capa", "Turquesa", [38, 160, 160], 0.4), ink("capa", "Verde agua", [100, 180, 150], 0.4),
        ink("capa", "Arena", [205, 168, 105], 0.4), ink("capa", "Coral", [232, 110, 90], 0.4)] },
    Palette { id: "embers", name: "Brasas", fallback: &[], inks: &[
        ink("capa", "Granate", [110, 30, 40], 0.4), ink("capa", "Rojo fuego", [200, 50, 45], 0.4),
        ink("capa", "Naranja tostado", [224, 138, 60], 0.4), ink("capa", "Ámbar", [232, 168, 40], 0.4),
        ink("capa", "Ciruela", [90, 40, 90], 0.4), ink("capa", "Carbón", [50, 45, 45], 0.4)] },
    Palette { id: "india_ink", name: "Tinta china", fallback: &[], inks: &[
        ink("capa", "Negro fino", [28, 28, 32], 0.4), ink("capa", "Gris oscuro", [72, 72, 78], 0.4),
        ink("capa", "Gris medio", [122, 122, 128], 0.4), ink("capa", "Gris claro", [168, 168, 174], 0.4)] },
    // Cuatro colores clásicos; el amarillo queda como quinto bolígrafo opcional.
    Palette { id: "ballpoint", name: "Bolígrafo", fallback: &[], inks: &[
        ink("capa", "Negro", [28, 30, 33], 0.35),
        ink("capa", "Azul", [22, 58, 155], 0.35),
        ink("capa", "Rojo", [198, 36, 48], 0.35),
        ink("capa", "Verde", [16, 118, 67], 0.35),
        ink("capa", "Amarillo", [224, 180, 0], 0.35),
    ] },
];
pub static SPIRO: InkPlan = InkPlan { parts: &[], fallback: &[], palettes: SPIRO_PALETTES, default_count: 3, cycle: true };

#[cfg(test)]
mod tests {
    use super::*;
    fn roles(palette: &str, count: usize) -> Vec<&'static str> {
        let choice = InkChoice { palette: palette.into(), count };
        ["contorno", "trama", "sombra", "vegetación", "terreno", "tejados"].iter().map(|n| SCENES.ink_for(&choice, 0, n).role).collect()
    }
    #[test]
    fn scene_roles_degrade_gracefully_with_fewer_pens() {
        // Grabado: tejas y sombras viajan con la trama.
        assert_eq!(roles("lime_indigo", 1), ["línea"; 6]);
        assert_eq!(roles("lime_indigo", 2), ["línea", "línea", "línea", "vegetación", "línea", "línea"]);
        assert_eq!(roles("lime_indigo", 3), ["línea", "trama", "trama", "vegetación", "trama", "trama"]);
        assert_eq!(roles("lime_indigo", 4), ["línea", "trama", "trama", "vegetación", "terreno", "trama"]);
        // Cartel: la sombra arrojada es lo segundo; las tejas, lo tercero.
        assert_eq!(roles("poster", 2), ["línea", "línea", "sombra", "línea", "línea", "línea"]);
        assert_eq!(roles("poster", 3), ["línea", "línea", "sombra", "línea", "línea", "tejado"]);
        assert_eq!(roles("poster", 5), ["línea", "línea", "sombra", "vegetación", "terreno", "tejado"]);
        assert_eq!(SCENES.default_choice(), InkChoice { palette: "poster".into(), count: 5 });
        // Fuera de rango se acota; un juego desconocido vuelve al predeterminado.
        assert_eq!(roles("lime_indigo", 9), roles("lime_indigo", 4));
        assert_eq!(SCENES.normalized(&InkChoice { palette: "nada".into(), count: 0 }), SCENES.default_choice());
    }
    #[test]
    fn ballpoint_palettes_share_classic_colors_and_yellow_is_optional() {
        let expected = [[28, 30, 33], [22, 58, 155], [198, 36, 48], [16, 118, 67], [224, 180, 0]];
        for plan in [&SCENES, &DENSE, &DOMES, &SPIRO] {
            let four = InkChoice { palette: "ballpoint".into(), count: 4 };
            assert_eq!(plan.active(&four).iter().map(|ink| ink.color).collect::<Vec<_>>(), expected[..4]);
            let five = InkChoice { count: 5, ..four.clone() };
            assert_eq!(plan.active(&five).last().unwrap().name, "Amarillo");
            assert_eq!(plan.active(&five).last().unwrap().color, expected[4]);
        }
        for plan in [&SCENES, &DENSE, &DOMES] {
            for count in 1..=5 {
                let choice = InkChoice { palette: "ballpoint".into(), count };
                for name in ["contorno", "detalle", "tejados", "gente", "vegetación", "agua", "sombra", "trama", "rótulos", "terreno"] {
                    assert!(plan.active(&choice).contains(&plan.ink_for(&choice, 0, name)));
                }
            }
            let choice = InkChoice { palette: "ballpoint".into(), count: 4 };
            assert_eq!(plan.ink_for(&choice, 0, "contorno").color, expected[0]);
            assert_eq!(plan.ink_for(&choice, 0, "sombra").color, expected[1]);
            assert_eq!(plan.ink_for(&choice, 0, "tejados").color, expected[2]);
            assert_eq!(plan.ink_for(&choice, 0, "vegetación").color, expected[3]);
            assert_eq!(plan.ink_for(&choice, 0, "terreno").color, expected[0]);
            let five = InkChoice { count: 5, ..choice };
            assert_eq!(plan.ink_for(&five, 0, "terreno").color, expected[4]);
        }
        let choice = InkChoice { palette: "ballpoint".into(), count: 4 };
        assert_eq!(DENSE.ink_for(&choice, 0, "agua").color, expected[1]);
        assert_eq!(DENSE.ink_for(&choice, 0, "gente").color, expected[2]);
        assert_eq!(DENSE.ink_for(&choice, 0, "rótulos").color, expected[0]);
    }

    #[test]
    fn palettes_are_well_formed() {
        for plan in [&SCENES, &SPIRO, &DENSE, &DOMES] {
            for palette in plan.palettes {
                assert!(!palette.inks.is_empty() && plan.palettes.iter().filter(|p| p.id == palette.id).count() == 1);
                let mut colors: Vec<_> = palette.inks.iter().map(|i| i.color).collect(); colors.sort(); colors.dedup();
                assert_eq!(colors.len(), palette.inks.len(), "{}", palette.name);
                // Ninguna tinta se pierde sobre papel blanco.
                for ink in palette.inks { assert!(ink.color.iter().map(|c| *c as u32).sum::<u32>() < 600, "{}", ink.name); }
            }
            assert!(plan.default_count <= plan.palettes[0].inks.len());
        }
        for palette in SCENES.palettes {
            let roles: Vec<_> = palette.inks.iter().map(|i| i.role).collect();
            let known = ["línea", "vegetación", "trama", "terreno", "sombra", "tejado"];
            assert_eq!(roles[0], "línea", "{}", palette.name);
            assert!(roles.iter().all(|r| known.contains(r)) && (1..roles.len()).all(|i| !roles[..i].contains(&roles[i])), "{}", palette.name);
        }
    }
    #[test]
    fn dense_roles_degrade_gracefully_from_seven_pens_to_one() {
        let names = ["contorno", "gente", "vegetación", "agua", "trama", "rótulos", "terreno", "tejados"];
        let roles = |count: usize| -> Vec<&'static str> {
            let choice = InkChoice { palette: "festival".into(), count };
            names.iter().map(|n| DENSE.ink_for(&choice, 0, n).role).collect()
        };
        assert_eq!(roles(1), ["línea"; 8]);
        assert_eq!(roles(2), ["línea", "gente", "línea", "línea", "línea", "gente", "línea", "gente"]);
        assert_eq!(roles(4), ["línea", "gente", "vegetación", "agua", "línea", "gente", "línea", "gente"]);
        assert_eq!(roles(5), ["línea", "gente", "vegetación", "agua", "trama", "gente", "trama", "gente"]);
        assert_eq!(roles(7), ["línea", "gente", "vegetación", "agua", "trama", "rótulos", "terreno", "gente"]);
        assert_eq!(DENSE.default_choice(), InkChoice { palette: "festival".into(), count: 5 });
        for palette in DENSE.palettes {
            let roles: Vec<_> = palette.inks.iter().map(|i| i.role).collect();
            assert_eq!(roles[0], "línea", "{}", palette.name);
            assert!((5..=7).contains(&roles.len()), "{}", palette.name);
            assert!((1..roles.len()).all(|i| !roles[..i].contains(&roles[i])), "{}", palette.name);
        }
    }
    #[test]
    fn spiro_layers_cycle_over_active_inks() {
        let choice = InkChoice { palette: "ocean".into(), count: 2 };
        let names: Vec<_> = (1..=5).map(|n| format!("Capa {n}")).collect();
        let pens = SPIRO.assign(&choice, names.iter().map(String::as_str));
        let colors: Vec<_> = names.iter().map(|n| pens[n].color).collect();
        assert_eq!(colors, [[27, 48, 92], [46, 110, 170], [27, 48, 92], [46, 110, 170], [27, 48, 92]]);
    }
}
