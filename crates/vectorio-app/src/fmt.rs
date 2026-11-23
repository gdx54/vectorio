//! Números, tiempos y distancias con el separador decimal del idioma activo.

fn spanish() -> bool {
    rust_i18n::locale().starts_with("es")
}

/// Número con `decimals` decimales: coma en español, punto en inglés.
pub fn decimal(x: f64, decimals: usize) -> String {
    let s = format!("{x:.decimals$}");
    if spanish() { s.replace('.', ",") } else { s }
}

/// «45 s», «3 min 05 s», «1 h 02 min». Las abreviaturas valen en los dos idiomas.
pub fn duration(seconds: f64) -> String {
    let s = seconds.ceil().max(0.0) as u64;
    if s >= 3600 { format!("{} h {:02} min", s / 3600, s / 60 % 60) }
    else if s >= 60 { format!("{} min {:02} s", s / 60, s % 60) }
    else { format!("{s} s") }
}

/// Estimación redondeada para leer de un vistazo, con segundos en trabajos cortos:
/// «8 s», «2 min 10 s», «12 min», «1 h 05 min». Diseño y Salida usan la misma.
pub fn estimate(seconds: f64) -> String {
    let s = seconds.ceil().max(0.0) as u64;
    if s < 600 { duration(s as f64) }
    else { let m = (seconds / 60.0).round() as u64; if m >= 60 { format!("{} h {:02} min", m / 60, m % 60) } else { format!("{m} min") } }
}

pub fn distance(mm: f64) -> String {
    if mm >= 1000.0 { format!("{} m", decimal(mm / 1000.0, 1)) } else { format!("{mm:.0} mm") }
}

pub fn pct(n: usize, total: usize) -> String {
    format!("{} %", decimal(n as f64 * 100.0 / total.max(1) as f64, 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decimal_separator_follows_the_language_and_units_are_plain() {
        let _lock = crate::i18n::locale_lock();
        rust_i18n::set_locale("es");
        assert_eq!((decimal(1.25, 1).as_str(), distance(1500.0).as_str(), pct(1, 8).as_str()), ("1,2", "1,5 m", "12,5 %"));
        rust_i18n::set_locale("en");
        assert_eq!((distance(1500.0).as_str(), pct(1, 8).as_str(), duration(3725.0).as_str()), ("1.5 m", "12.5 %", "1 h 02 min"));
        assert_eq!((duration(45.0).as_str(), duration(185.0).as_str(), distance(80.0).as_str()), ("45 s", "3 min 05 s", "80 mm"));
        rust_i18n::set_locale("es");
    }
}
