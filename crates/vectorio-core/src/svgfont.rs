//! Tipografías de trazo único en formato SVG 1.1 (las de Hershey Text de Evil Mad
//! Scientist). Van incluidas en el binario y se leen una sola vez.
//!
//! Créditos y licencia (SIL Open Font License) en `fonts/OFL.txt` y en los metadatos de
//! cada archivo. Las fuentes Hershey derivan del trabajo del Dr. A. V. Hershey.

use crate::geom::{p, Polyline, P};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Glifo normalizado: altura de mayúsculas = 1, línea base en y = 0, y hacia arriba.
#[derive(Clone, Debug)]
pub struct Glyph {
    pub adv: f64,
    pub strokes: Vec<Polyline>,
}

pub struct SvgFont {
    pub glyphs: HashMap<char, Glyph>,
    /// Altura de x relativa a la de mayúsculas.
    pub xh: f64,
    pub space: f64,
}

pub struct FontInfo {
    pub src: &'static str,
    /// Distancia máxima (en alturas de mayúscula) para enlazar dos letras; 0 = no se enlaza.
    pub join: f64,
}

pub const FONTS: [FontInfo; 8] = [
    FontInfo { src: include_str!("../fonts/EMSCapitol.svg"), join: 0.5 },
    FontInfo { src: include_str!("../fonts/EMSAllure.svg"), join: 0.5 },
    FontInfo { src: include_str!("../fonts/EMSPepita.svg"), join: 0.3 },
    FontInfo { src: include_str!("../fonts/EMSNeato.svg"), join: 0.0 },
    FontInfo { src: include_str!("../fonts/EMSCasualHand.svg"), join: 0.0 },
    FontInfo { src: include_str!("../fonts/EMSTech.svg"), join: 0.0 },
    FontInfo { src: include_str!("../fonts/EMSSociety.svg"), join: 0.4 },
    FontInfo { src: include_str!("../fonts/HersheyScript1.svg"), join: 0.45 },
];

pub fn font(i: usize) -> &'static SvgFont {
    static PARSED: OnceLock<Vec<SvgFont>> = OnceLock::new();
    &PARSED.get_or_init(|| FONTS.iter().map(|f| parse(f.src)).collect())[i]
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let start = tag.find(&key)? + key.len();
    let end = tag[start..].find('"')? + start;
    Some(&tag[start..end])
}

fn decode(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let Some(j) = rest[i..].find(';') else {
            out.push_str(&rest[i..]);
            return out;
        };
        let ent = &rest[i + 1..i + j];
        let ch = match ent {
            "amp" => Some('&'),
            "apos" => Some('\''),
            "quot" => Some('"'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            _ if ent.starts_with("#x") || ent.starts_with("#X") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        if let Some(c) = ch {
            out.push(c);
        }
        rest = &rest[i + j + 1..];
    }
    out.push_str(rest);
    out
}

enum Tok {
    Cmd(char),
    Num(f64),
}

fn tokens(d: &str) -> Vec<Tok> {
    let b = d.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if c.is_ascii_alphabetic() {
            out.push(Tok::Cmd(c));
            i += 1;
        } else if c == '-' || c == '+' || c == '.' || c.is_ascii_digit() {
            let start = i;
            i += 1;
            let mut dot = c == '.';
            while i < b.len() {
                let ch = b[i] as char;
                if ch.is_ascii_digit() {
                    i += 1;
                } else if ch == '.' && !dot {
                    dot = true;
                    i += 1;
                } else if (ch == 'e' || ch == 'E') && i + 1 < b.len() {
                    i += 1;
                    if b[i] == b'-' || b[i] == b'+' {
                        i += 1;
                    }
                } else {
                    break;
                }
            }
            if let Ok(v) = d[start..i].parse() {
                out.push(Tok::Num(v));
            }
        } else {
            i += 1;
        }
    }
    out
}

/// Datos de trazado SVG → polilíneas (M, L, H, V, C, Q, Z; absolutos y relativos).
pub fn path(d: &str) -> Vec<Polyline> {
    let toks = tokens(d);
    let mut out: Vec<Polyline> = Vec::new();
    let mut cur = P::default();
    let mut start = P::default();
    let mut cmd = 'M';
    let mut i = 0;
    let num = |i: &mut usize| -> Option<f64> {
        match toks.get(*i) {
            Some(Tok::Num(v)) => {
                *i += 1;
                Some(*v)
            }
            _ => None,
        }
    };
    while i < toks.len() {
        if let Tok::Cmd(c) = toks[i] {
            cmd = c;
            i += 1;
            if c == 'Z' || c == 'z' {
                if let Some(l) = out.last_mut() {
                    l.push(start);
                }
                cur = start;
                continue;
            }
        }
        let rel = cmd.is_ascii_lowercase();
        let base = if rel { cur } else { P::default() };
        match cmd.to_ascii_uppercase() {
            'M' => {
                let (Some(x), Some(y)) = (num(&mut i), num(&mut i)) else { break };
                cur = base + p(x, y);
                start = cur;
                out.push(vec![cur]);
                // Pares extra tras M son L implícitas.
                cmd = if rel { 'l' } else { 'L' };
            }
            'L' => {
                let (Some(x), Some(y)) = (num(&mut i), num(&mut i)) else { break };
                cur = base + p(x, y);
                push(&mut out, cur);
            }
            'H' => {
                let Some(x) = num(&mut i) else { break };
                cur = p(if rel { cur.x + x } else { x }, cur.y);
                push(&mut out, cur);
            }
            'V' => {
                let Some(y) = num(&mut i) else { break };
                cur = p(cur.x, if rel { cur.y + y } else { y });
                push(&mut out, cur);
            }
            'C' => {
                let v: Vec<f64> = (0..6).filter_map(|_| num(&mut i)).collect();
                if v.len() < 6 {
                    break;
                }
                let (c1, c2, e) = (base + p(v[0], v[1]), base + p(v[2], v[3]), base + p(v[4], v[5]));
                let s = cur;
                for k in 1..=10 {
                    let t = k as f64 / 10.0;
                    let u = 1.0 - t;
                    push(&mut out, s * (u * u * u) + c1 * (3.0 * u * u * t) + c2 * (3.0 * u * t * t) + e * (t * t * t));
                }
                cur = e;
            }
            'Q' => {
                let v: Vec<f64> = (0..4).filter_map(|_| num(&mut i)).collect();
                if v.len() < 4 {
                    break;
                }
                let (c, e) = (base + p(v[0], v[1]), base + p(v[2], v[3]));
                let s = cur;
                for k in 1..=8 {
                    let t = k as f64 / 8.0;
                    let u = 1.0 - t;
                    push(&mut out, s * (u * u) + c * (2.0 * u * t) + e * (t * t));
                }
                cur = e;
            }
            _ => {
                // Comando no admitido: salta sus números.
                while num(&mut i).is_some() {}
                if matches!(toks.get(i), Some(Tok::Num(_))) {
                    i += 1;
                }
            }
        }
    }
    out.retain(|l| l.len() >= 2);
    out
}

fn push(out: &mut Vec<Polyline>, q: P) {
    match out.last_mut() {
        Some(l) => l.push(q),
        None => out.push(vec![q]),
    }
}

pub fn parse(src: &str) -> SvgFont {
    let default_adv: f64 = src.find("<font ").and_then(|i| attr(&src[i..], "horiz-adv-x")).and_then(|v| v.parse().ok()).unwrap_or(400.0);
    let mut raw: HashMap<char, (f64, Vec<Polyline>)> = HashMap::new();
    let mut rest = src;
    while let Some(i) = rest.find("<glyph") {
        let tag_end = rest[i..].find('>').map(|j| i + j + 1).unwrap_or(rest.len());
        let tag = &rest[i..tag_end];
        rest = &rest[tag_end..];
        let Some(u) = attr(tag, "unicode").map(decode) else { continue };
        let mut chars = u.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else { continue };
        let adv = attr(tag, "horiz-adv-x").and_then(|v| v.parse().ok()).unwrap_or(default_adv);
        let strokes = attr(tag, "d").map(path).unwrap_or_default();
        raw.insert(c, (adv, strokes));
    }
    let ymax = |c: char| raw.get(&c).map(|(_, s)| s.iter().flatten().map(|q| q.y).fold(f64::MIN, f64::max)).filter(|v| *v > 0.0);
    let cap = ymax('H').or(ymax('I')).unwrap_or(700.0);
    let xh = ymax('x').or(ymax('o')).unwrap_or(cap * 0.5) / cap;
    let glyphs = raw
        .into_iter()
        .map(|(c, (adv, strokes))| {
            let strokes = strokes.into_iter().map(|l| l.into_iter().map(|q| q * (1.0 / cap)).collect()).collect();
            (c, Glyph { adv: adv / cap, strokes })
        })
        .collect::<HashMap<_, _>>();
    let space = glyphs.get(&' ').map(|g| g.adv).unwrap_or(0.3);
    SvgFont { glyphs, xh, space }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_fonts_parse_with_spanish_letters() {
        for i in 0..FONTS.len() {
            let f = font(i);
            assert!(f.glyphs.len() > 150, "fuente {i}");
            assert!(f.xh > 0.2 && f.xh < 0.9, "fuente {i}: xh {}", f.xh);
            for c in "aeñáéíóú¿¡AZ09".chars() {
                assert!(f.glyphs.contains_key(&c), "fuente {i}: falta {c}");
            }
            let h = &f.glyphs[&'H'];
            assert!(!h.strokes.is_empty() && h.adv > 0.2);
        }
    }

    #[test]
    fn path_parses_curves_and_relative() {
        let l = path("M 0 0 L 10 0 C 10 5 5 10 0 10 m 1 1 l 2 0 z");
        assert_eq!(l.len(), 2);
        assert!((l[0].last().unwrap().y - 10.0).abs() < 1e-9);
        assert_eq!(l[1].len(), 3);
    }
}
