//! Piezas compartidas para guardar datos grandes en el documento sin copiarlos en cada
//! paso de deshacer: [`Shared`] (un `Arc` que se compara por dirección antes que por
//! contenido) y base64 para incrustar imágenes en el JSON.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::Arc;

/// Dato inmutable compartido entre copias del documento. Clonar es barato y comparar
/// dos copias del mismo dato no recorre su contenido.
#[derive(Debug, Default)]
pub struct Shared<T>(pub Arc<T>);

impl<T> Shared<T> {
    pub fn new(value: T) -> Self {
        Shared(Arc::new(value))
    }
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared(self.0.clone())
    }
}

impl<T> std::ops::Deref for Shared<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: PartialEq> PartialEq for Shared<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || *self.0 == *other.0
    }
}

impl<T: Serialize> Serialize for Shared<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Shared<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        T::deserialize(d).map(Shared::new)
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn base64_decode(text: &str) -> Result<Vec<u8>, String> {
    let value = |c: u8| -> Result<u32, String> {
        Ok(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err("base64 no válido".into()),
        } as u32)
    };
    let clean: Vec<u8> = text.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    let mut out = Vec::with_capacity(clean.len() / 4 * 3);
    for chunk in clean.chunks(4) {
        let data: Vec<u8> = chunk.iter().copied().take_while(|c| *c != b'=').collect();
        if data.len() < 2 {
            return Err("base64 incompleto".into());
        }
        let mut n = 0u32;
        for (i, c) in data.iter().enumerate() {
            n |= value(*c)? << (18 - 6 * i);
        }
        for i in 0..data.len() - 1 {
            out.push((n >> (16 - 8 * i)) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips_every_length() {
        let data: Vec<u8> = (0..=255u8).collect();
        for n in [0, 1, 2, 3, 4, 5, 100, 256] {
            assert_eq!(base64_decode(&base64_encode(&data[..n])).unwrap(), &data[..n]);
        }
        assert_eq!(base64_encode(b"Vectorio"), "VmVjdG9yaW8=");
    }

    #[test]
    fn shared_copies_compare_without_reading_and_by_value() {
        let a = Shared::new(vec![1.0f32, f32::NAN]);
        // NaN ≠ NaN por valor, pero una copia del mismo dato es igual a sí misma.
        assert_eq!(a, a.clone());
        assert_ne!(a, Shared::new(vec![1.0, f32::NAN]));
        assert_eq!(Shared::new(vec![2]), Shared::new(vec![2]));
    }
}
