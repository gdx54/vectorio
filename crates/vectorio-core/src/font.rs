//! Texto «a mano» de un solo trazo para el plotter.
//!
//! Hay dos familias de letra:
//! - Las tipografías SVG de trazo único de Hershey Text (ver `svgfont`), varias de ellas
//!   cursivas pensadas para enlazar letras.
//! - La tipografía propia de Vectorio, descrita aquí con trazos en una rejilla: línea base
//!   en y = 0, altura de x = 5, mayúsculas y ascendentes = 8, descendentes hasta −3. Un punto
//!   precedido de `!` es una esquina (no se suaviza); `~` al final cierra el trazo.
//!
//! Sobre cualquiera de ellas se aplican, de forma determinista (semilla), las imperfecciones
//! de la escritura real: cada letra cambia un poco de tamaño, giro y altura, la línea base
//! ondula, el pulso tiembla y, en las cursivas, las letras de una palabra se unen en un
//! único trazo para no levantar el rotulador.

use crate::geom::{catmull_rom, p, Polyline, Rng, P};
use crate::svgfont::{self, SvgFont};
use serde::{Deserialize, Serialize};

const CAP: f64 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontStyle {
    /// EMS Capitol: cursiva enlazada, redonda y legible.
    Capitol,
    /// EMS Allure: caligrafía inclinada y elegante.
    Allure,
    /// EMS Pepita: letra a mano informal.
    Pepita,
    /// EMS Neato: letra escolar de imprenta.
    Neato,
    /// EMS Casual Hand: rotulador rápido.
    Casual,
    /// EMS Tech: rotulación de arquitecto.
    Arquitecto,
    /// EMS Society: cursiva de firma con lazos.
    Society,
    /// Hershey Script 1: cursiva clásica de plotter.
    HersheyScript,
    /// Vectorio: curvas suaves con variación natural entre letras.
    Manuscrita,
    /// Vectorio: curvas suaves sin variación.
    Cuidada,
    /// Vectorio: segmentos rectos, aspecto de rotulación técnica.
    Tecnica,
}

impl FontStyle {
    pub const ALL: [FontStyle; 11] = [
        FontStyle::Capitol,
        FontStyle::Allure,
        FontStyle::Society,
        FontStyle::HersheyScript,
        FontStyle::Pepita,
        FontStyle::Casual,
        FontStyle::Neato,
        FontStyle::Arquitecto,
        FontStyle::Manuscrita,
        FontStyle::Cuidada,
        FontStyle::Tecnica,
    ];
    pub fn label(self) -> &'static str {
        match self {
            FontStyle::Capitol => "Capitol · cursiva unida",
            FontStyle::Allure => "Allure · caligrafía",
            FontStyle::Society => "Society · firma",
            FontStyle::HersheyScript => "Hershey Script · clásica",
            FontStyle::Pepita => "Pepita · a mano",
            FontStyle::Casual => "Casual · rotulador",
            FontStyle::Neato => "Neato · escolar",
            FontStyle::Arquitecto => "Arquitecto · técnica",
            FontStyle::Manuscrita => "Vectorio · manuscrita",
            FontStyle::Cuidada => "Vectorio · cuidada",
            FontStyle::Tecnica => "Vectorio · técnica",
        }
    }
    /// Índice en `svgfont::FONTS`, o `None` para la tipografía propia.
    fn svg_index(self) -> Option<usize> {
        Some(match self {
            FontStyle::Capitol => 0,
            FontStyle::Allure => 1,
            FontStyle::Pepita => 2,
            FontStyle::Neato => 3,
            FontStyle::Casual => 4,
            FontStyle::Arquitecto => 5,
            FontStyle::Society => 6,
            FontStyle::HersheyScript => 7,
            _ => return None,
        })
    }
    pub fn is_builtin(self) -> bool {
        self.svg_index().is_none()
    }
    /// La letra está pensada para enlazarse (tiene sentido «Unir letras»).
    pub fn joins(self) -> bool {
        self.svg_index().map(|i| svgfont::FONTS[i].join > 0.0).unwrap_or(false)
    }
    /// Admite variación de pulso (todas salvo las dos de Vectorio sin variación).
    pub fn has_jitter(self) -> bool {
        !matches!(self, FontStyle::Cuidada | FontStyle::Tecnica)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Align {
    Left,
    Center,
    Right,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextSpec {
    pub text: String,
    /// Altura de las mayúsculas en mm.
    pub size_mm: f64,
    pub style: FontStyle,
    /// Inclinación en grados (positivo = cursiva hacia la derecha).
    pub slant_deg: f64,
    /// Pulso: cantidad de imperfección manuscrita, 0–2.
    pub jitter: f64,
    /// Espaciado adicional entre letras, en fracción de la altura.
    pub letter_spacing: f64,
    /// Interlineado como múltiplo de la altura.
    pub line_height: f64,
    pub align: Align,
    /// Ancho máximo de línea en mm (0 = sin ajuste automático).
    pub wrap_mm: f64,
    pub seed: u64,
    /// Unir las letras de cada palabra en un solo trazo (letras cursivas).
    #[serde(default = "yes")]
    pub connect: bool,
}

impl Default for TextSpec {
    fn default() -> Self {
        TextSpec {
            text: "Hola, Vectorio".into(),
            size_mm: 10.0,
            style: FontStyle::Capitol,
            slant_deg: 0.0,
            jitter: 0.6,
            letter_spacing: 0.0,
            line_height: 1.7,
            align: Align::Left,
            wrap_mm: 0.0,
            seed: 1,
            connect: true,
        }
    }
}

#[derive(Clone, Copy)]
enum Accent {
    Acute,
    Grave,
    Diaeresis,
    Tilde,
    Circumflex,
    Cedilla,
}

fn decompose(c: char) -> (char, Option<Accent>) {
    use Accent::*;
    match c {
        'á' => ('a', Some(Acute)),
        'é' => ('e', Some(Acute)),
        'í' => ('i', Some(Acute)),
        'ó' => ('o', Some(Acute)),
        'ú' => ('u', Some(Acute)),
        'Á' => ('A', Some(Acute)),
        'É' => ('E', Some(Acute)),
        'Í' => ('I', Some(Acute)),
        'Ó' => ('O', Some(Acute)),
        'Ú' => ('U', Some(Acute)),
        'à' => ('a', Some(Grave)),
        'è' => ('e', Some(Grave)),
        'ì' => ('i', Some(Grave)),
        'ò' => ('o', Some(Grave)),
        'ù' => ('u', Some(Grave)),
        'À' => ('A', Some(Grave)),
        'È' => ('E', Some(Grave)),
        'Ò' => ('O', Some(Grave)),
        'ä' => ('a', Some(Diaeresis)),
        'ë' => ('e', Some(Diaeresis)),
        'ï' => ('i', Some(Diaeresis)),
        'ö' => ('o', Some(Diaeresis)),
        'ü' => ('u', Some(Diaeresis)),
        'Ü' => ('U', Some(Diaeresis)),
        'Ö' => ('O', Some(Diaeresis)),
        'Ä' => ('A', Some(Diaeresis)),
        'ñ' => ('n', Some(Tilde)),
        'Ñ' => ('N', Some(Tilde)),
        'ã' => ('a', Some(Tilde)),
        'õ' => ('o', Some(Tilde)),
        'â' => ('a', Some(Circumflex)),
        'ê' => ('e', Some(Circumflex)),
        'î' => ('i', Some(Circumflex)),
        'ô' => ('o', Some(Circumflex)),
        'û' => ('u', Some(Circumflex)),
        'ç' => ('c', Some(Cedilla)),
        'Ç' => ('C', Some(Cedilla)),
        _ => (c, None),
    }
}

const BOWL_A: &str = "4,4 3,5 1.5,5 0.5,3.8 0.5,1.2 1.5,0 3,0 4,1.2";
const BOWL_B: &str = "0.5,3 1.5,4.6 3,5 4.2,4 4.4,2.5 4,1 3,0 1.5,0 0.5,1";
const O_UP: &str = "2.8,8 1.2,7.2 0.3,5.4 0.3,2.6 1.2,0.8 2.8,0 4.4,0.8 5.3,2.6 5.3,5.4 4.4,7.2 ~";
const P_BOWL: &str = "0.6,8 !2.6,8 3.8,7.4 4.2,6 3.8,4.6 !2.6,4 0.6,4";

/// Ancho de avance y trazos de cada glifo.
fn glyph(c: char) -> Option<(f64, &'static [&'static str])> {
    Some(match c {
        ' ' => (3.0, &[]),
        'a' => (5.0, &[BOWL_A, "4,5 4,1 4.5,0"]),
        'b' => (5.0, &["0.5,8 0.5,0", BOWL_B]),
        'c' => (4.6, &["4,4.2 3,5 1.5,5 0.4,3.6 0.4,1.4 1.5,0 3,0 4,0.8"]),
        'd' => (5.0, &[BOWL_A, "4,8 4,1 4.5,0"]),
        'e' => (4.6, &["0.5,2.5 !4,2.5 3.9,3.8 3,5 1.5,5 0.4,3.6 0.4,1.4 1.5,0 3,0 4,0.8"]),
        'f' => (3.6, &["3.5,7.6 2.8,8 2,7.8 1.6,7 1.6,0", "0.4,5 3.2,5"]),
        'g' => (5.0, &[BOWL_A, "4,5 4,-1.5 3.2,-2.8 1.8,-3 0.6,-2.2"]),
        'h' => (4.8, &["0.5,8 0.5,0", "0.5,3 1.5,4.6 2.8,5 3.8,4.4 4,3.2 4,0"]),
        'i' => (2.0, &["1,5 1,0", "1,6.8 1,7"]),
        'j' => (3.0, &["2,5 2,-1.8 1.4,-2.9 0.2,-2.7", "2,6.8 2,7"]),
        'k' => (4.2, &["0.5,8 0.5,0", "3.6,5 0.6,2", "1.6,3 3.8,0"]),
        'l' => (2.4, &["1,8 1,0.8 1.5,0 2,0.1"]),
        'm' => (6.4, &["0.5,5 0.5,0", "0.5,3.4 1.3,4.7 2.2,5 3,4.5 3.2,3.4 3.2,0", "3.2,3.4 4,4.7 4.9,5 5.7,4.5 5.9,3.4 5.9,0"]),
        'n' => (4.6, &["0.5,5 0.5,0", "0.5,3.4 1.5,4.7 2.7,5 3.7,4.4 3.9,3.2 3.9,0"]),
        'o' => (4.8, &["2.3,5 0.9,4.4 0.3,2.5 0.9,0.6 2.3,0 3.7,0.6 4.3,2.5 3.7,4.4 ~"]),
        'p' => (5.0, &["0.5,5 0.5,-3", BOWL_B]),
        'q' => (5.0, &[BOWL_A, "4,5 4,-3 4.6,-2.4"]),
        'r' => (3.6, &["0.5,5 0.5,0", "0.5,3 1.4,4.5 2.6,5 3.4,4.8"]),
        's' => (4.2, &["3.6,4.3 2.6,5 1.3,5 0.5,4.2 0.7,3.2 2,2.6 3.3,2 3.8,1 3.2,0.2 1.8,0 0.4,0.6"]),
        't' => (3.4, &["1.6,7 1.6,1 2.1,0.1 3,0.1", "0.3,5 3,5"]),
        'u' => (4.6, &["0.5,5 0.5,1.6 0.9,0.4 2,0 3.2,0.4 3.9,1.6", "3.9,5 3.9,0"]),
        'v' => (4.0, &["0.2,5 !2,0 3.8,5"]),
        'w' => (5.6, &["0.2,5 !1.5,0 !2.8,4 !4.1,0 5.4,5"]),
        'x' => (4.0, &["0.4,5 3.6,0", "3.6,5 0.4,0"]),
        'y' => (4.2, &["0.3,5 2.1,0.6", "3.8,5 1.8,-1.6 1.1,-2.7 0.2,-2.9"]),
        'z' => (4.2, &["0.4,5 !3.7,5 !0.3,0 3.8,0"]),

        'A' => (5.0, &["0.2,0 !2.5,8 4.8,0", "1.1,2.8 3.9,2.8"]),
        'B' => (5.0, &["0.6,0 0.6,8", "0.6,8 !2.6,8 3.6,7.5 3.9,6.3 3.4,4.9 !2.3,4.3 0.6,4.3", "0.6,4.3 !2.5,4.3 3.8,3.7 4.3,2.3 3.9,0.8 !2.8,0 0.6,0"]),
        'C' => (5.4, &["5,6.8 4,7.8 2.8,8 1.4,7.4 0.5,5.8 0.3,4 0.6,2 1.5,0.6 2.9,0 4.1,0.2 5,1.1"]),
        'D' => (5.4, &["0.6,0 0.6,8", "0.6,8 !2.2,8 3.8,7.2 4.7,5.6 4.9,4 4.6,2.2 3.7,0.8 !2.2,0 0.6,0"]),
        'E' => (4.6, &["4.2,8 !0.6,8 !0.6,0 4.2,0", "0.6,4.2 3.4,4.2"]),
        'F' => (4.4, &["4.2,8 !0.6,8 0.6,0", "0.6,4.2 3.4,4.2"]),
        'G' => (5.6, &["5,6.8 4,7.8 2.8,8 1.4,7.4 0.5,5.8 0.3,4 0.6,2 1.5,0.6 2.9,0 4.2,0.3 !5.1,1.2 !5.1,3.6 3.2,3.6"]),
        'H' => (5.2, &["0.6,8 0.6,0", "4.6,8 4.6,0", "0.6,4.2 4.6,4.2"]),
        'I' => (2.0, &["1,8 1,0"]),
        'J' => (4.2, &["3.6,8 3.6,1.8 3.1,0.5 2,0 0.9,0.4 0.3,1.5"]),
        'K' => (4.8, &["0.6,8 0.6,0", "4.4,8 0.7,3.2", "2,4.6 4.6,0"]),
        'L' => (4.2, &["0.6,8 !0.6,0 4,0"]),
        'M' => (6.0, &["0.5,0 !0.9,8 !3,2.4 !5.1,8 5.5,0"]),
        'N' => (5.2, &["0.6,0 !0.6,8 !4.6,0 4.6,8"]),
        'O' => (5.6, &[O_UP]),
        'P' => (4.6, &["0.6,0 0.6,8", P_BOWL]),
        'Q' => (5.6, &[O_UP, "3.2,1.6 5.4,-0.4"]),
        'R' => (4.8, &["0.6,0 0.6,8", P_BOWL, "2.4,4 4.4,0"]),
        'S' => (4.8, &["4.4,7 3.5,7.9 2.2,8 1,7.4 0.6,6.2 1.2,5 2.6,4.3 3.9,3.5 4.5,2.2 4,0.8 2.6,0 1.2,0.2 0.3,1"]),
        'T' => (4.8, &["0.2,8 4.6,8", "2.4,8 2.4,0"]),
        'U' => (5.2, &["0.6,8 0.6,2.4 1.1,0.8 2.6,0 4.1,0.8 4.6,2.4 4.6,8"]),
        'V' => (5.0, &["0.2,8 !2.5,0 4.8,8"]),
        'W' => (6.4, &["0.2,8 !1.6,0 !3.2,6 !4.8,0 6.2,8"]),
        'X' => (4.8, &["0.4,8 4.4,0", "4.4,8 0.4,0"]),
        'Y' => (4.8, &["0.3,8 !2.4,4.2 4.5,8", "2.4,4.2 2.4,0"]),
        'Z' => (4.9, &["0.5,8 !4.3,8 !0.4,0 4.5,0"]),

        '0' => (4.4, &["2.2,7.6 0.9,6.8 0.3,4.8 0.3,2.8 0.9,0.8 2.2,0 3.5,0.8 4.1,2.8 4.1,4.8 3.5,6.8 ~"]),
        '1' => (3.6, &["0.8,6 !2.4,7.6 2.4,0"]),
        '2' => (4.6, &["0.5,6.2 1.2,7.3 2.4,7.6 3.6,7.1 4,5.9 3.6,4.6 !0.4,0 4.2,0"]),
        '3' => (4.5, &["0.5,6.8 1.5,7.5 2.6,7.6 3.6,7 3.8,5.8 3.2,4.7 !1.8,4.2 3.4,3.7 4.1,2.4 3.8,1 2.7,0.1 1.4,0 0.3,0.7"]),
        '4' => (4.6, &["3.2,0 !3.2,7.6 !0.2,2.4 4.4,2.4"]),
        '5' => (4.6, &["3.9,7.6 !1,7.6 !0.6,4.2 1.8,4.7 3,4.6 3.9,3.8 4.2,2.4 3.8,0.9 2.6,0 1.3,0.1 0.3,0.8"]),
        '6' => (4.5, &["3.7,7 2.8,7.6 1.6,7.4 0.7,6.2 0.3,4 0.5,1.8 1.3,0.3 2.4,0 3.5,0.4 4.1,1.6 4,2.9 3.2,4 2,4.2 1,3.6 0.4,2.6"]),
        '7' => (4.6, &["0.3,7.6 !4.3,7.6 1.6,0"]),
        '8' => (4.4, &["2.2,4.2 3.3,4.8 3.8,5.9 3.4,7.1 2.2,7.6 1,7.1 0.6,5.9 1.1,4.8 2.2,4.2 3.4,3.6 4,2.3 3.6,0.7 2.2,0 0.8,0.7 0.4,2.3 1,3.6 ~"]),
        '9' => (4.5, &["4,4.8 3.2,3.6 2,3.4 0.9,4 0.4,5.3 0.8,6.8 2,7.6 3.2,7.3 3.9,6 4.1,4 3.8,1.6 3,0.3 1.8,0 0.7,0.5"]),

        '.' => (1.6, &["0.8,0 0.8,0.2"]),
        ',' => (1.6, &["0.9,0.3 0.9,0 0.4,-1.2"]),
        '!' => (2.0, &["1,8 1,2.4", "1,0 1,0.2"]),
        '¡' => (2.0, &["1,4.8 1,5", "1,3.2 1,-2.4"]),
        '?' => (4.2, &["0.4,6.4 1.1,7.5 2.3,7.9 3.5,7.4 3.9,6.2 3.4,5 2.3,4.2 2.1,2.6", "2.1,0 2.1,0.2"]),
        '¿' => (4.2, &["2.1,4.8 2.1,5", "2.1,3.4 1.9,1.8 0.8,1 0.3,-0.2 0.7,-1.4 1.9,-1.9 3.1,-1.5 3.8,-0.4"]),
        ':' => (1.6, &["0.8,4.6 0.8,4.8", "0.8,0 0.8,0.2"]),
        ';' => (1.6, &["0.9,4.6 0.9,4.8", "0.9,0.3 0.9,0 0.4,-1.2"]),
        '-' => (3.2, &["0.4,3.2 2.8,3.2"]),
        '–' | '—' => (5.0, &["0.4,3.2 4.6,3.2"]),
        '_' => (4.0, &["0,-0.8 4,-0.8"]),
        '\'' | '’' | '‘' => (1.4, &["0.8,8 0.6,6"]),
        '"' | '“' | '”' | '«' | '»' => (2.4, &["0.8,8 0.6,6", "1.8,8 1.6,6"]),
        '(' => (2.4, &["2,8.4 0.9,6.8 0.4,4 0.9,1 2,-0.6"]),
        ')' => (2.4, &["0.4,8.4 1.5,6.8 2,4 1.5,1 0.4,-0.6"]),
        '/' => (3.4, &["0.2,-0.5 3.2,8.4"]),
        '+' => (4.0, &["0.4,3.2 3.6,3.2", "2,1.6 2,4.8"]),
        '=' => (3.8, &["0.4,4 3.4,4", "0.4,2.2 3.4,2.2"]),
        '&' => (4.8, &["4.4,0 !1.2,5 0.9,6.4 1.4,7.6 2.4,7.9 3.2,7.3 3.3,6.2 2.4,5.2 0.9,4 0.3,2.4 0.8,0.7 2,0 3.2,0.4 4.2,2.2"]),
        '@' => (5.8, &["3.6,3 3.3,4.2 2.4,4.6 1.6,4 1.4,2.8 1.9,2 2.9,2.1 !3.6,3 3.7,2.2 4.3,1.8 5,2.4 5.3,3.8 4.8,5.6 3.4,6.5 1.8,6.3 0.6,5 0.3,3.2 0.8,1.4 2.2,0.4 3.8,0.5"]),
        '#' => (5.0, &["1.4,0.5 2,7", "3.2,0.5 3.8,7", "0.4,2.4 4.6,2.4", "0.6,5 4.8,5"]),
        '*' => (4.0, &["2,6.8 2,4", "0.8,6 3.2,4.8", "3.2,6 0.8,4.8"]),
        '%' => (4.8, &["0.4,0 4.4,7.6", "1.2,7.6 0.4,6.8 1.2,6 2,6.8 ~", "3.6,1.6 2.8,0.8 3.6,0 4.4,0.8 ~"]),
        '$' => (4.4, &["3.6,6.4 2.6,7 1.3,6.9 0.6,6 0.9,4.9 2.2,4.3 3.4,3.6 3.8,2.4 3.2,1.1 2,0.8 0.6,1.3", "2.2,8 2.2,0"]),
        '€' => (5.0, &["4.6,6.8 3.8,7.6 2.6,7.6 1.5,6.8 1,5.2 1,2.4 1.5,0.8 2.6,0 3.8,0 4.6,0.8", "0.2,4.6 3.4,4.6", "0.2,3 3.2,3"]),
        '<' => (3.6, &["3.2,5.4 !0.4,3.2 3.2,1"]),
        '>' => (3.6, &["0.4,5.4 !3.2,3.2 0.4,1"]),
        _ => return None,
    })
}

struct Stroke {
    pts: Vec<(P, bool)>,
    closed: bool,
}

fn parse(s: &str) -> Stroke {
    let mut pts = Vec::new();
    let mut closed = false;
    for tok in s.split_whitespace() {
        if tok == "~" {
            closed = true;
            continue;
        }
        let (corner, body) = match tok.strip_prefix('!') {
            Some(b) => (true, b),
            None => (false, tok),
        };
        if let Some((x, y)) = body.split_once(',') {
            if let (Ok(x), Ok(y)) = (x.parse::<f64>(), y.parse::<f64>()) {
                pts.push((p(x, y), corner));
            }
        }
    }
    Stroke { pts, closed }
}

fn accent_strokes(a: Accent, cx: f64, top: f64) -> Vec<Stroke> {
    let s = |v: Vec<(f64, f64)>| Stroke { pts: v.into_iter().map(|(x, y)| (p(x, y), false)).collect(), closed: false };
    match a {
        Accent::Acute => vec![s(vec![(cx - 0.4, top + 1.2), (cx + 0.6, top + 2.4)])],
        Accent::Grave => vec![s(vec![(cx - 0.6, top + 2.4), (cx + 0.4, top + 1.2)])],
        Accent::Diaeresis => vec![
            s(vec![(cx - 0.9, top + 1.6), (cx - 0.9, top + 1.8)]),
            s(vec![(cx + 0.9, top + 1.6), (cx + 0.9, top + 1.8)]),
        ],
        Accent::Tilde => vec![s(vec![(cx - 1.4, top + 1.4), (cx - 0.6, top + 2.0), (cx + 0.4, top + 1.5), (cx + 1.2, top + 2.1)])],
        Accent::Circumflex => vec![Stroke {
            pts: vec![(p(cx - 0.9, top + 1.2), false), (p(cx, top + 2.2), true), (p(cx + 0.9, top + 1.2), false)],
            closed: false,
        }],
        Accent::Cedilla => vec![s(vec![(cx, 0.0), (cx, -0.8), (cx + 0.6, -1.4), (cx - 0.1, -2.0)])],
    }
}

/// Trazos de un carácter en unidades de la rejilla, con su ancho de avance.
fn char_strokes(c: char) -> (f64, Vec<Stroke>) {
    let (base, accent) = decompose(c);
    let Some((w, defs)) = glyph(base).or_else(|| glyph('?')) else {
        return (3.0, vec![]);
    };
    let mut strokes: Vec<Stroke> = defs.iter().map(|d| parse(d)).collect();
    if let Some(a) = accent {
        if base == 'i' {
            // i sin punto bajo la tilde.
            strokes.retain(|s| {
                let (a, b) = (s.pts[0].0, s.pts[s.pts.len() - 1].0);
                a.dist(b) > 0.5
            });
        }
        let top = if base.is_uppercase() { CAP } else { 5.0 };
        strokes.extend(accent_strokes(a, w / 2.0, top));
    }
    (w, strokes)
}

fn render_stroke(s: &Stroke, smooth: bool) -> Polyline {
    if s.pts.len() < 2 {
        return s.pts.iter().map(|q| q.0).collect();
    }
    if !smooth {
        let mut out: Polyline = s.pts.iter().map(|q| q.0).collect();
        if s.closed {
            out.push(out[0]);
        }
        return out;
    }
    let has_corner = s.pts.iter().any(|q| q.1);
    if !has_corner {
        let pts: Vec<P> = s.pts.iter().map(|q| q.0).collect();
        return catmull_rom(&pts, 6, s.closed);
    }
    // Tramos entre esquinas, suavizados por separado y unidos en la esquina.
    let mut out: Polyline = Vec::new();
    let mut run: Vec<P> = vec![s.pts[0].0];
    let flush = |run: &Vec<P>, out: &mut Polyline| {
        let seg = if run.len() >= 3 { catmull_rom(run, 6, false) } else { run.clone() };
        if out.is_empty() {
            out.extend(seg);
        } else {
            out.extend(seg.into_iter().skip(1));
        }
    };
    for &(q, corner) in &s.pts[1..] {
        run.push(q);
        if corner {
            flush(&run, &mut out);
            run = vec![q];
        }
    }
    if run.len() >= 2 {
        flush(&run, &mut out);
    }
    if s.closed {
        out.push(out[0]);
    }
    out
}

/// Origen de los glifos de un estilo. Todo en unidades de altura de mayúsculas, y hacia
/// arriba, línea base en 0.
enum Face {
    Builtin { smooth: bool, noise: bool },
    Svg { font: &'static SvgFont, join: f64 },
}

impl Face {
    fn of(style: FontStyle) -> Face {
        match style.svg_index() {
            Some(i) => Face::Svg { font: svgfont::font(i), join: svgfont::FONTS[i].join },
            None => Face::Builtin { smooth: style != FontStyle::Tecnica, noise: style == FontStyle::Manuscrita },
        }
    }
    fn xh(&self) -> f64 {
        match self {
            Face::Builtin { .. } => 5.0 / CAP,
            Face::Svg { font, .. } => font.xh,
        }
    }
    fn join(&self) -> f64 {
        match self {
            Face::Builtin { .. } => 0.0,
            Face::Svg { join, .. } => *join,
        }
    }
    fn adv(&self, c: char) -> f64 {
        if let Face::Svg { font, .. } = self {
            if let Some(g) = font.glyphs.get(&c) {
                return g.adv;
            }
            if c == ' ' {
                return font.space;
            }
        }
        (char_strokes(c).0 + 0.9) / CAP
    }
    fn glyph(&self, c: char, rng: &mut Rng, jitter: f64) -> (f64, Vec<Polyline>) {
        if let Face::Svg { font, .. } = self {
            if let Some(g) = font.glyphs.get(&c) {
                return (g.adv, g.strokes.clone());
            }
            if c == ' ' {
                return (font.space, vec![]);
            }
        }
        let (smooth, noise) = match self {
            Face::Builtin { smooth, noise } => (*smooth, *noise),
            Face::Svg { .. } => (true, false),
        };
        let (w, mut strokes) = char_strokes(c);
        if noise && jitter > 0.0 {
            for s in &mut strokes {
                for (q, _) in &mut s.pts {
                    *q = *q + p(rng.gauss() * 0.08 * jitter, rng.gauss() * 0.08 * jitter);
                }
            }
        }
        let lines = strokes
            .iter()
            .map(|s| render_stroke(s, smooth))
            .filter(|l| l.len() >= 2)
            .map(|l| l.into_iter().map(|q| q * (1.0 / CAP)).collect())
            .collect();
        ((w + 0.9) / CAP, lines)
    }
}

fn line_width(face: &Face, text: &str, spacing: f64) -> f64 {
    text.chars().map(|c| face.adv(c) + spacing).sum()
}

fn wrap_lines(spec: &TextSpec, face: &Face) -> Vec<String> {
    let cap = spec.size_mm.max(0.5);
    let mut out = Vec::new();
    for para in spec.text.split('\n') {
        if spec.wrap_mm <= 0.0 {
            out.push(para.to_string());
            continue;
        }
        let mut line = String::new();
        for word in para.split(' ') {
            let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if !line.is_empty() && line_width(face, &candidate, spec.letter_spacing) * cap > spec.wrap_mm {
                out.push(std::mem::take(&mut line));
                line = word.to_string();
            } else {
                line = candidate;
            }
        }
        out.push(line);
    }
    out
}

/// Dirección unitaria desde `from` hacia el primer punto de `pts` que esté a cierta distancia.
fn direction(from: P, pts: impl Iterator<Item = P>) -> P {
    let mut last = from;
    for q in pts {
        last = q;
        if q.dist(from) > 0.04 {
            break;
        }
    }
    let d = last - from;
    let l = d.len();
    if l < 1e-9 {
        p(1.0, 0.0)
    } else {
        d * (1.0 / l)
    }
}

/// Puente curvo entre la salida de una letra y la entrada de la siguiente (sin los extremos).
fn bridge(a: P, ta: P, b: P, tb: P) -> Vec<P> {
    let d = a.dist(b);
    if d < 0.015 {
        return vec![];
    }
    let c1 = a + ta * (d * 0.45);
    let c2 = b - tb * (d * 0.45);
    (1..9)
        .map(|k| {
            let t = k as f64 / 9.0;
            let u = 1.0 - t;
            a * (u * u * u) + c1 * (3.0 * u * u * t) + c2 * (3.0 * u * t * t) + b * (t * t * t)
        })
        .collect()
}

/// Salida de la letra: el extremo de trazo más a la derecha cerca de la línea de la x y del
/// borde derecho. Devuelve (trazo, el extremo es el final).
fn find_exit(strokes: &[Polyline], x0: f64, adv: f64, xh: f64) -> Option<(usize, bool)> {
    let mut best: Option<(usize, bool, f64)> = None;
    for (i, s) in strokes.iter().enumerate() {
        for (end, q) in [(false, s[0]), (true, s[s.len() - 1])] {
            if q.y < -0.08 || q.y > xh * 1.15 || q.x < x0 + adv - 0.22 {
                continue;
            }
            if best.map(|b| q.x > b.2).unwrap_or(true) {
                best = Some((i, end, q.x));
            }
        }
    }
    best.map(|b| (b.0, b.1))
}

/// Entrada de la letra: el extremo más cercano a la salida anterior, dentro de la tolerancia,
/// sin contar la propia salida de esta letra.
fn find_entry(strokes: &[Polyline], from: P, tol: f64, exit: Option<(usize, bool)>) -> Option<(usize, bool)> {
    let mut best: Option<(usize, bool, f64)> = None;
    for (i, s) in strokes.iter().enumerate() {
        for (end, q) in [(false, s[0]), (true, s[s.len() - 1])] {
            if exit == Some((i, end)) {
                continue;
            }
            // Si la salida está en este trazo, la entrada solo puede ser su otro extremo.
            if let Some((ei, eend)) = exit {
                if ei == i && end == eend {
                    continue;
                }
            }
            // Subir mucho desde la salida para entrar deforma la letra (una «u» parecería una «o»).
            if q.y - from.y > 0.3 {
                continue;
            }
            let d = q.dist(from);
            if d <= tol && best.map(|b| d < b.2).unwrap_or(true) {
                best = Some((i, end, d));
            }
        }
    }
    best.map(|b| (b.0, b.1))
}

/// Remuestrea para que ningún tramo supere `max` (así el temblor se reparte bien).
fn resample(l: &[P], max: f64) -> Polyline {
    let mut out = Vec::with_capacity(l.len() * 2);
    out.push(l[0]);
    for w in l.windows(2) {
        let n = (w[0].dist(w[1]) / max).ceil().max(1.0) as usize;
        for k in 1..=n {
            out.push(w[0].lerp(w[1], k as f64 / n as f64));
        }
    }
    out
}

/// Trazos del texto en mm, coordenadas locales con y hacia abajo y origen en la esquina
/// superior izquierda de la primera línea.
pub fn layout(spec: &TextSpec) -> Vec<Polyline> {
    let cap = spec.size_mm.max(0.5);
    let face = Face::of(spec.style);
    let jitter = if spec.style.has_jitter() { spec.jitter.clamp(0.0, 2.0) } else { 0.0 };
    let xh = face.xh();
    let join = if spec.connect { face.join() } else { 0.0 };
    let shear = spec.slant_deg.to_radians().tan();
    let lines = wrap_lines(spec, &face);
    let widths: Vec<f64> = lines.iter().map(|l| line_width(&face, l, spec.letter_spacing)).collect();
    let max_w = widths.iter().cloned().fold(0.0, f64::max);
    let mut rng = Rng::new(spec.seed);
    let mut out = Vec::new();

    for (li, line) in lines.iter().enumerate() {
        let mut x = match spec.align {
            Align::Left => 0.0,
            Align::Center => (max_w - widths[li]) / 2.0,
            Align::Right => max_w - widths[li],
        };
        let wave_phase = rng.f() * std::f64::consts::TAU;
        let wave_len = 6.0 + rng.f() * 3.0;
        let mut row: Vec<Polyline> = Vec::new();
        // Salida pendiente de enlazar: (índice en `row`, punto, dirección).
        let mut pending: Option<(usize, P, P)> = None;

        for c in line.chars() {
            let (adv, mut strokes) = face.glyph(c, &mut rng, jitter);
            // Imperfecciones de cada letra: tamaño, giro, altura y avance.
            let (size, rot, lift, adv_k) = if jitter > 0.0 {
                (
                    1.0 + rng.gauss() * 0.035 * jitter,
                    rng.gauss() * 0.03 * jitter,
                    rng.gauss() * 0.022 * jitter + (x * std::f64::consts::TAU / wave_len + wave_phase).sin() * 0.035 * jitter,
                    1.0 + rng.gauss() * 0.03 * jitter,
                )
            } else {
                (1.0, 0.0, 0.0, 1.0)
            };
            let center = p(adv / 2.0, xh / 2.0);
            for s in &mut strokes {
                for q in s.iter_mut() {
                    *q = ((*q - center).rotate(rot) * size + center) + p(x, lift);
                }
            }
            strokes.retain(|s| s.len() >= 2);

            let letter = c.is_alphabetic();
            let exit = if join > 0.0 && letter { find_exit(&strokes, x, adv, xh) } else { None };
            let entry = match pending {
                Some((_, e, _)) if letter => find_entry(&strokes, e, join, exit),
                _ => None,
            };
            // Orienta: la entrada al principio de su trazo y la salida al final del suyo.
            if let Some((i, true)) = entry {
                strokes[i].reverse();
            }
            if let Some((i, false)) = exit {
                if entry.map(|e| e.0) != Some(i) {
                    strokes[i].reverse();
                }
            }
            let entry_i = entry.map(|e| e.0);
            let exit_i = exit.map(|e| e.0);
            let mut exit_row: Option<usize> = None;

            if let (Some(ei), Some((ri, e, te))) = (entry_i, pending) {
                let s = &strokes[ei];
                let ts = direction(s[0], s.iter().copied());
                let b = bridge(e, te, s[0], ts);
                row[ri].extend(b);
                row[ri].extend(s.iter().copied());
                if exit_i == Some(ei) {
                    exit_row = Some(ri);
                }
            }
            for (i, s) in strokes.iter().enumerate() {
                if Some(i) == entry_i || Some(i) == exit_i {
                    continue;
                }
                row.push(s.clone());
            }
            if let Some(xi) = exit_i {
                if exit_row.is_none() {
                    row.push(strokes[xi].clone());
                    exit_row = Some(row.len() - 1);
                }
            }
            pending = exit_row.map(|ri| {
                let l = &row[ri];
                let end = l[l.len() - 1];
                (ri, end, direction(end, l.iter().rev().copied()) * -1.0)
            });
            if !letter {
                pending = None;
            }
            x += adv * adv_k + spec.letter_spacing;
        }

        // Temblor del pulso: un campo suave y continuo, igual a ambos lados de cada unión.
        let (ph1, ph2) = (rng.f() * 10.0, rng.f() * 10.0);
        let amp = 0.012 * jitter;
        for l in row {
            let l = if jitter > 0.0 { resample(&l, 0.03) } else { l };
            out.push(
                l.into_iter()
                    .map(|q| {
                        let (mut gx, mut gy) = (q.x, q.y);
                        if amp > 0.0 {
                            gx += amp * ((q.y * 14.0 + ph1).sin() + 0.5 * (q.x * 23.0 + ph2).sin());
                            gy += amp * ((q.x * 13.0 + ph2).sin() + 0.5 * (q.y * 21.0 + ph1).sin());
                        }
                        let sx = gx + gy * shear;
                        p(sx * cap, (li as f64 * spec.line_height + 1.0 - gy) * cap)
                    })
                    .collect(),
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_renders_ascii_and_spanish() {
        let text: String = (33u8..127).map(|b| b as char).chain("áéíóúñÑüÜ¿¡çÇ€".chars()).collect();
        for style in FontStyle::ALL {
            let spec = TextSpec { text: text.clone(), style, ..Default::default() };
            let strokes = layout(&spec);
            assert!(strokes.len() > 80, "{style:?}: {}", strokes.len());
            assert!(strokes.iter().all(|s| s.len() >= 2 && s.iter().all(|q| q.x.is_finite() && q.y.is_finite())), "{style:?}");
        }
    }

    #[test]
    fn cursive_joins_letters_of_a_word() {
        for style in [FontStyle::Capitol, FontStyle::Society, FontStyle::HersheyScript, FontStyle::Allure] {
            let base = TextSpec { text: "nunca minimum".into(), style, jitter: 0.0, ..Default::default() };
            let joined = layout(&base).len();
            let loose = layout(&TextSpec { connect: false, ..base }).len();
            assert!(joined * 10 <= loose * 7, "{style:?}: unidas {joined}, sueltas {loose}");
        }
    }

    #[test]
    fn size_matches_cap_height() {
        let spec = TextSpec { text: "H".into(), style: FontStyle::Capitol, jitter: 0.0, size_mm: 20.0, ..Default::default() };
        let ys: Vec<f64> = layout(&spec).iter().flatten().map(|q| q.y).collect();
        let h = ys.iter().cloned().fold(f64::MIN, f64::max) - ys.iter().cloned().fold(f64::MAX, f64::min);
        assert!((h - 20.0).abs() < 1.5, "altura {h}");
    }

    #[test]
    fn handwriting_is_deterministic_per_seed() {
        for style in [FontStyle::Manuscrita, FontStyle::Capitol] {
            let s = TextSpec { style, ..Default::default() };
            assert_eq!(layout(&s), layout(&s));
            assert_ne!(layout(&s), layout(&TextSpec { seed: 2, ..s.clone() }));
        }
    }
}
