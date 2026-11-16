use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InkSet {
    Graphite,
    Cmyk,
    Primary,
    Thermal,
    Earth,
}
impl InkSet {
    pub const ALL: [Self; 5] = [
        Self::Graphite,
        Self::Cmyk,
        Self::Primary,
        Self::Thermal,
        Self::Earth,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Graphite => "Grafito",
            Self::Cmyk => "Cuatricromía",
            Self::Primary => "Colores primarios",
            Self::Thermal => "Térmica",
            Self::Earth => "Tierra y añil",
        }
    }
    pub fn pens(self) -> Vec<crate::Pen> {
        let colors: &[(&str, [u8; 3])] = match self {
            Self::Graphite => &[("Negro fino", [28, 28, 32])],
            Self::Cmyk => &[
                ("Cian", [0, 160, 200]),
                ("Magenta", [210, 45, 140]),
                ("Amarillo", [247, 200, 30]),
                ("Negro fino", [28, 28, 32]),
            ],
            Self::Primary => &[
                ("Rojo", [214, 48, 49]),
                ("Amarillo", [247, 200, 30]),
                ("Azul", [9, 92, 196]),
                ("Negro fino", [28, 28, 32]),
            ],
            Self::Thermal => &[
                ("Índigo", [53, 33, 108]),
                ("Azul", [9, 92, 196]),
                ("Verde", [0, 148, 96]),
                ("Naranja", [245, 124, 32]),
                ("Amarillo", [247, 200, 30]),
            ],
            Self::Earth => &[
                ("Marrón", [126, 75, 48]),
                ("Naranja", [203, 107, 57]),
                ("Índigo", [53, 33, 108]),
            ],
        };
        colors
            .iter()
            .map(|(name, color)| crate::Pen::new(name, *color, 0.4))
            .collect()
    }
    /// Reutiliza tintas idénticas sin cambiar las que ya usan otros objetos.
    pub fn apply(self, document: &mut crate::Document) -> Vec<usize> {
        self.pens()
            .into_iter()
            .map(|pen| {
                if let Some(i) = document
                    .pens
                    .iter()
                    .position(|p| p.color == pen.color && p.width_mm == pen.width_mm)
                {
                    i
                } else {
                    let i = document.pens.len();
                    document.pens.push(pen);
                    i
                }
            })
            .collect()
    }
    pub fn matches(self, map: &ReadingMap, palette: &[crate::Pen]) -> bool {
        let expected = self.pens();
        map.pens.len() == expected.len()
            && map.pens.iter().zip(expected).all(|(i, p)| {
                palette
                    .get(*i)
                    .is_some_and(|v| v.color == p.color && v.width_mm == p.width_mm)
            })
    }
}
