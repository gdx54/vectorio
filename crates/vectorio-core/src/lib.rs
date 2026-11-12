//! Vectorio · núcleo sin interfaz.
//!
//! Todo se mide en milímetros sobre la hoja, con y hacia abajo. Los objetos del
//! documento son paramétricos (texto, formas, trazados) y se convierten en capas de
//! plotter (una por rotulador) con `Document::layers`.

pub mod adjust;
pub mod doc;
pub mod fill;
pub mod font;
pub mod geom;
pub mod mix;
pub mod optimize;
pub mod output;
pub mod photo;
pub mod raster;
pub mod shapes;
pub mod shared;
pub mod svg_in;
pub mod svg_out;
pub mod svgfont;
pub mod trace;
pub mod vectorize;

pub use adjust::ColorAdjust;
pub use doc::{Document, Geometry, Item, Paper, Pen, PlotLayer, PlotSettings};
pub use fill::{Fill, FillStyle};
pub use font::{Align, FontStyle, TextSpec};
pub use geom::{p, BBox, Placement, Polyline, P};
pub use photo::Photo;
pub use shapes::Shape;
pub use shared::Shared;
pub use trace::{mix_cross, mix_cross_fan, mix_interleave, mix_relay, Coverage, DotShape, Relay, TraceKey, TraceParams, TraceStyle, Traced, Vibe};
