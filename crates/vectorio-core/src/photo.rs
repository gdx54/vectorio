//! Foto viva: la imagen original y los ajustes con que se separaron sus colores. Las
//! tintas de la foto son objetos del documento ([`crate::trace::Traced`]) que apuntan
//! a ella con `Item::group`; así se puede volver a separar la foto sin importarla de
//! nuevo.

use crate::shared::{base64_decode, base64_encode, Shared};
use crate::vectorize::VecParams;
use image::DynamicImage;
use serde::{Deserialize, Serialize};

/// Lado mayor con que se guarda la foto en el documento, px.
pub const STORED_PX: u32 = 1600;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Photo {
    pub id: u64,
    pub name: String,
    pub image: Shared<EncodedImage>,
    pub params: VecParams,
    /// Tamaño elegido al importar, % del área útil del papel.
    pub size_pct: f64,
}

/// Imagen comprimida (JPEG) tal como se guarda en el archivo.
#[derive(Clone, Debug, PartialEq)]
pub struct EncodedImage {
    pub bytes: Vec<u8>,
}

impl EncodedImage {
    /// Reduce la foto a [`STORED_PX`] y la comprime para guardarla.
    pub fn from_image(img: &DynamicImage) -> Result<Self, String> {
        let rgb = crate::vectorize::working_image(img, STORED_PX);
        let mut bytes = Vec::new();
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 92);
        rgb.write_with_encoder(encoder).map_err(|e| e.to_string())?;
        Ok(EncodedImage { bytes })
    }

    pub fn decode(&self) -> Result<DynamicImage, String> {
        image::load_from_memory(&self.bytes).map_err(|e| e.to_string())
    }
}

impl Serialize for EncodedImage {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&base64_encode(&self.bytes))
    }
}

impl<'de> Deserialize<'de> for EncodedImage {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        base64_decode(&text).map(|bytes| EncodedImage { bytes }).map_err(serde::de::Error::custom)
    }
}
