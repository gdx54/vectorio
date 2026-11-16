//! Una lámina de una receta con parámetros a mano, para revisarla sola:
//! `cargo run -p vectorio-core --example muestra_receta --release -- festival_city ciudad.svg 3 event=fair stories=1`.
//! Los valores numéricos van como número, `si`/`no` como interruptor y el resto como texto.
mod support;
use vectorio_core::{p, BBox, generate::{self, Detail, GenContext, ParamValue, Params}};

fn value(text: &str) -> ParamValue {
    match text {
        "si" => ParamValue::Bool(true),
        "no" => ParamValue::Bool(false),
        _ => text.parse().map(ParamValue::Number).unwrap_or_else(|_| ParamValue::Text(text.into())),
    }
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let id = args.first().map(String::as_str).unwrap_or("festival_city");
    let path = args.get(1).cloned().unwrap_or_else(|| "muestra.svg".into());
    let seed = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let generator = generate::all().iter().find(|g| g.id() == id).ok_or_else(|| std::io::Error::other("receta desconocida"))?;
    let mut params = Params::default();
    let specs = generator.params();
    if args.iter().any(|arg| arg == "--random") {
        generator.randomize(&mut vectorio_core::geom::Rng::new(seed), &mut params, &std::collections::BTreeSet::new());
    }
    for pair in args.iter().skip(3) {
        if let Some((key, text)) = pair.split_once('=') {
            // Las opciones de una lista van siempre como texto, aunque parezcan números.
            let listed = specs.iter().any(|spec| matches!(spec, generate::ParamSpec::Choice { id, .. } if *id == key));
            params.set(key, if listed { ParamValue::Text(text.into()) } else { value(text) });
        }
    }
    let ctx = GenContext { area: BBox { min: p(5.0, 5.0), max: p(195.0, 195.0) }, seed, pen_mm: 0.35, detail: Detail::Final };
    let start = std::time::Instant::now();
    let output = generator.generate(&params, &ctx).map_err(std::io::Error::other)?;
    eprintln!("{} · {seed}: {:?}, {:.2} m {}", generator.name(), start.elapsed(), output.total_m(), output.warning.as_deref().unwrap_or(""));
    let mut svg = String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" width="200mm" height="200mm" viewBox="0 0 200 200"><rect width="100%" height="100%" fill="white"/>"#);
    let mut choice=generator.inks().default_choice();
    if let Some(palette)=args.iter().find_map(|arg|arg.strip_prefix("palette=")){choice.palette=palette.into();}
    if let Some(count)=args.iter().find_map(|arg|arg.strip_prefix("pens=")).and_then(|s|s.parse().ok()){choice.count=count;}
    support::colored_paths(&mut svg, &output,generator.inks(),&choice);
    svg.push_str("</svg>");
    std::fs::write(path, svg)
}
