//! La cartela se plotea: letra de trazo único, rampa real y escala gráfica.
use super::*;
pub(super) fn text(out: &mut Vec<Polyline>, message: &str, origin: P, size: f64, width: f64) {
    let spec = crate::font::TextSpec {
        text: message.into(),
        style: crate::font::FontStyle::Tecnica,
        size_mm: size,
        jitter: 0.,
        wrap_mm: width,
        frame_height_mm: size * 1.8,
        connect: false,
        ..Default::default()
    };
    let lines = crate::font::layout(&spec);
    let bounds = BBox::of(&lines);
    for line in lines {
        out.push(line.iter().map(|q| *q - bounds.min + origin).collect());
    }
}
pub(super) fn add(
    out: &mut [Vec<Polyline>],
    map: &ReadingMap,
    values: &fields::Values,
    d: &Drawing,
    s: &Settings,
    paper: &crate::Paper,
    row: usize,
    kind: &PassKind,
    legend_scale: f64,
) {
    let mut legends = vec![vec![]; out.len()];
    let actual = out;
    let out = &mut legends;
    let width = paper.width_mm - 2. * paper.margin_mm;
    let y = paper.height_mm - paper.margin_mm - (row + 1) as f64 * 26. + 2.;
    let x = paper.margin_mm;
    let area = BBox {
        min: p(x, y),
        max: p(paper.width_mm - paper.margin_mm, y + 22.),
    };
    let bar_width = (width * 0.42).min(90.);
    let n = out.len();
    let annotation = if map.encoding == Encoding::Optical {
        optical::annotation_slot(map)
    } else {
        0
    };
    let categorical = matches!(
        kind,
        PassKind::Reading(ReadingParams {
            effect: ReadingEffect::Unfold { .. }
                | ReadingEffect::Anaglyph { .. }
                | ReadingEffect::Section { .. }
                | ReadingEffect::Ridges { .. }
                | ReadingEffect::Suggestive { .. }
                | ReadingEffect::ApparentRidges { .. }
                | ReadingEffect::PrincipalFlow { .. }
                | ReadingEffect::Spin { .. },
            ..
        })
    ) || map.measure == Measure::Objects;
    if map.encoding == Encoding::Optical {
        optical::legend(out, map, values, x, y, bar_width);
    } else {
        for (i, lines) in out.iter_mut().enumerate() {
            let (tlo, thi) = if map.ramp == Ramp::Diverging && !categorical {
                let left = (n / 2).max(1);
                if i < left {
                    (
                        0.465 * i as f64 / left as f64,
                        0.465 * (i + 1) as f64 / left as f64,
                    )
                } else {
                    (
                        0.535 + 0.465 * (i - left) as f64 / (n - left) as f64,
                        0.535 + 0.465 * (i - left + 1) as f64 / (n - left) as f64,
                    )
                }
            } else {
                (i as f64 / n as f64, (i + 1) as f64 / n as f64)
            };
            let a = x + tlo * bar_width;
            let b = x + thi * bar_width;
            for j in 0..7 {
                lines.push(vec![
                    p(a, y + 4. + j as f64 * 0.35),
                    p(b, y + 4. + j as f64 * 0.35),
                ]);
            }
            let label = match kind {
                PassKind::Reading(ReadingParams { effect: ReadingEffect::Voxels { .. }, .. }) => ["X", "Y", "Z"][i].into(),
                PassKind::Reading(ReadingParams {
                    effect: ReadingEffect::Unfold { .. },
                    ..
                }) => ["Corte", "Monte", "Valle"]
                    .get(i)
                    .unwrap_or(&"Tinta")
                    .to_string(),
                PassKind::Reading(ReadingParams {
                    effect: ReadingEffect::Anaglyph { .. },
                    ..
                }) => ["Izq.", "Der."].get(i).unwrap_or(&"Tinta").to_string(),
                PassKind::Reading(ReadingParams {
                    effect: ReadingEffect::Section { .. },
                    ..
                }) => ["Vista", "Oculta", "Corte"]
                    .get(i)
                    .unwrap_or(&"Tinta")
                    .to_string(),
                PassKind::Reading(ReadingParams {
                    effect: ReadingEffect::Ridges { .. },
                    ..
                }) => ["Crestas", "Valles"].get(i).unwrap_or(&"Tinta").to_string(),
                PassKind::Reading(ReadingParams {
                    effect:
                        ReadingEffect::Suggestive { .. }
                        | ReadingEffect::ApparentRidges { .. }
                        | ReadingEffect::PrincipalFlow { .. },
                    ..
                }) => "Forma".into(),
                PassKind::Reading(ReadingParams {
                    effect: ReadingEffect::Spin { angle_deg, copies },
                    ..
                }) => format!("{:.1} grados", angle_deg * (i * copies / n) as f64),
                _ if map.measure == Measure::Objects => {
                    let names = d
                        .mesh
                        .objects
                        .iter()
                        .enumerate()
                        .filter(|(j, _)| j % n == i)
                        .map(|(_, name)| name.as_str())
                        .collect::<Vec<_>>();
                    if names.is_empty() {
                        format!("Objeto {}", i + 1)
                    } else {
                        names.join(" / ")
                    }
                }
                _ => format!("{:.2}", values.raw_at(tlo)),
            };
            text(lines, &label, p(a, y + 7.), 1.5, (b - a).max(4.));
        }
    }
    let title = match kind {
        PassKind::Reading(ReadingParams { effect: ReadingEffect::Voxels { .. }, .. }) => "Caras X / Y / Z".into(),
        PassKind::Reading(ReadingParams {
            effect: ReadingEffect::PrincipalFlow { .. },
            ..
        }) => "Trazos de curvatura".into(),
        PassKind::Reading(ReadingParams {
            effect: ReadingEffect::Unfold { .. },
            ..
        }) => "Corte / monte / valle".into(),
        PassKind::Reading(ReadingParams {
            effect: ReadingEffect::Anaglyph { .. },
            ..
        }) => "Izquierda / derecha".into(),
        PassKind::Reading(ReadingParams {
            effect: ReadingEffect::Spin { .. },
            ..
        }) => "Estela de giro".into(),
        PassKind::Reading(ReadingParams {
            effect: ReadingEffect::Section { .. },
            ..
        }) => "Vista / oculta / seccion".into(),
        PassKind::Reading(ReadingParams {
            effect: ReadingEffect::Suggestive { .. },
            ..
        }) => "Contornos sugeridos".into(),
        PassKind::Reading(ReadingParams {
            effect: ReadingEffect::ApparentRidges { .. },
            ..
        }) => "Aristas aparentes".into(),
        _ => {
            if let Some(secondary) = map.secondary {
                format!(
                    "{:.1} {} + {:.1} {}",
                    map.weights[0],
                    map.measure.name(),
                    map.weights[1],
                    secondary.name()
                )
            } else {
                format!("{} [{}]", map.measure.name(), map.measure.units())
            }
        }
    };
    text(&mut out[annotation], &title, p(x, y), 2., bar_width);
    if !categorical {
        text(
            &mut out[annotation],
            &format!("{:.2}", values.raw_at(1.)),
            p(x + bar_width - 5., y + 10.),
            1.5,
            12.,
        );
    }
    let meta_x = x + bar_width + 8.;
    let meta_width = (width - bar_width - 8.).max(12.);
    text(
        &mut out[annotation],
        &format!("{} {}", d.mesh.name, map.date),
        p(meta_x, y),
        1.8,
        meta_width,
    );
    text(
        &mut out[annotation],
        &format!(
            "Camara {:.1}/{:.1}  Luz {:.1}/{:.1}",
            s.yaw, s.elevation, s.shadow.azimuth, s.shadow.elevation
        ),
        p(meta_x, y + 4.),
        1.5,
        meta_width,
    );
    let scale = 10f64.powf((20. / legend_scale.max(1e-9)).log10().floor());
    let mm = scale * legend_scale;
    if mm > 0.01 && mm < meta_width {
        out[0].push(vec![p(meta_x, y + 12.), p(meta_x + mm, y + 12.)]);
        for dx in [0., mm] {
            out[0].push(vec![p(meta_x + dx, y + 11.), p(meta_x + dx, y + 13.)]);
        }
        text(
            &mut out[annotation],
            &format!("{scale:.3} u / {mm:.1} mm"),
            p(meta_x, y + 15.),
            1.5,
            meta_width,
        );
    }
    for (dest, lines) in actual.iter_mut().zip(out.iter()) {
        dest.extend(encode::clip(lines, area));
    }
}
