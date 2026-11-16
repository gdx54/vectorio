//! OBJ local, sin materiales externos ni red. Polígonos simples por ear clipping.
use super::*;
use std::io::Read;

pub fn load_obj(path: &std::path::Path) -> Result<(Mesh, MeshReport), String> {
    let bytes = read_model(path, &|| false)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| "El OBJ no es texto UTF-8 válido.")?;
    parse_obj(
        text,
        path.file_stem().and_then(|s| s.to_str()).unwrap_or("OBJ"),
    )
}
pub fn parse_obj(text: &str, name: &str) -> Result<(Mesh, MeshReport), String> {
    parse_obj_cancellable(text, name, &|| false)
}
pub(super) fn parse_obj_cancellable(
    text: &str,
    name: &str,
    cancel: &dyn Fn() -> bool,
) -> Result<(Mesh, MeshReport), String> {
    if text.len() as u64 > MAX_FILE_BYTES {
        return Err("OBJ demasiado grande: máximo 512 MB.".into());
    }
    let mut mesh = Mesh {
        name: name.into(),
        vertices: vec![],
        triangles: vec![],
        objects: vec!["Modelo".into()],
        face_objects: vec![],
    };
    let mut owner = 0;
    for (line_no, line) in text.lines().enumerate() {
        if line_no % 4096 == 0 && cancel() {
            return Err("Cálculo cancelado.".into());
        }
        let err = || format!("OBJ: datos no válidos en la línea {}.", line_no + 1);
        if line.len() > 65_536 {
            return Err(err());
        }
        let mut words = line.split('#').next().unwrap_or("").split_whitespace();
        match words.next() {
            Some("v") => {
                let mut q = [0.; 3];
                for v in &mut q {
                    *v = words.next().ok_or_else(err)?.parse().map_err(|_| err())?;
                }
                mesh.vertices.push(q);
                if mesh.vertices.len() > MAX_VERTICES {
                    return Err("OBJ supera 15 millones de vértices.".into());
                }
            }
            Some("o" | "g") => {
                if mesh.objects.len() >= MAX_FACES {
                    return Err("Demasiados objetos OBJ.".into());
                }
                let label = words.collect::<Vec<_>>().join(" ");
                if label.len() > 4096 {
                    return Err(err());
                }
                mesh.objects.push(label);
                owner = mesh.objects.len() - 1;
            }
            Some("f") => {
                let indices = words
                    .map(|w| {
                        let i = w
                            .split('/')
                            .next()
                            .unwrap_or("")
                            .parse::<i64>()
                            .map_err(|_| err())?;
                        let n = mesh.vertices.len() as i64;
                        let i = if i < 0 { n + i } else { i - 1 };
                        if i < 0 || i >= n {
                            return Err(err());
                        }
                        Ok(i as usize)
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                if indices.len() < 3 || indices.len() > 256 {
                    return Err(format!(
                        "{} Las caras admiten entre 3 y 256 vértices.",
                        err()
                    ));
                }
                let tris = triangulate(&indices, &mesh.vertices).ok_or_else(|| {
                    format!(
                        "{} Triangula las caras no planas o autointersecadas en el modelador.",
                        err()
                    )
                })?;
                if mesh.triangles.len() + tris.len() > MAX_FACES {
                    return Err("OBJ supera 5 millones de triángulos.".into());
                }
                mesh.face_objects
                    .extend(std::iter::repeat_n(owner, tris.len()));
                mesh.triangles.extend(tris);
            }
            _ => {}
        }
    }
    mesh.validated_cancellable(cancel)
}
fn triangulate(ids: &[usize], v: &[[f64; 3]]) -> Option<Vec<[usize; 3]>> {
    if ids.len() == 3 {
        return Some(vec![[ids[0], ids[1], ids[2]]]);
    }
    let points: Vec<_> = ids.iter().map(|i| v3(v[*i])).collect();
    if points.iter().any(|p| !p.finite() || p.length() > 2e12) {
        return None;
    }
    let origin = points[0];
    let size = points
        .iter()
        .map(|p| (*p - origin).length())
        .fold(0., f64::max)
        .max(1e-12);
    let points: Vec<_> = points
        .into_iter()
        .map(|p| (p - origin) * (1. / size))
        .collect();
    let normal = (0..points.len())
        .fold(V3::default(), |s, i| {
            s + points[i].cross(points[(i + 1) % points.len()])
        })
        .normalized();
    if normal.length() < 0.5 || points.iter().any(|p| p.dot(normal).abs() > 1e-6) {
        return None;
    }
    let axis = if normal.x.abs() > normal.y.abs() && normal.x.abs() > normal.z.abs() {
        0
    } else if normal.y.abs() > normal.z.abs() {
        1
    } else {
        2
    };
    let p: Vec<_> = points
        .iter()
        .map(|p| match axis {
            0 => crate::p(p.y, p.z),
            1 => crate::p(p.x, p.z),
            _ => crate::p(p.x, p.y),
        })
        .collect();
    let cross = |a: crate::P, b: crate::P| a.x * b.y - a.y * b.x;
    let sign = crate::geom::signed_area(&p).signum();
    let mut ring: Vec<_> = (0..ids.len()).collect();
    let mut out = Vec::new();
    while ring.len() > 3 {
        let mut found = false;
        for i in 0..ring.len() {
            let a = ring[(i + ring.len() - 1) % ring.len()];
            let b = ring[i];
            let c = ring[(i + 1) % ring.len()];
            if cross(p[b] - p[a], p[c] - p[b]) * sign <= 1e-12 {
                continue;
            }
            if ring.iter().any(|j| {
                *j != a
                    && *j != b
                    && *j != c
                    && cross(p[b] - p[a], p[*j] - p[a]) * sign >= -1e-12
                    && cross(p[c] - p[b], p[*j] - p[b]) * sign >= -1e-12
                    && cross(p[a] - p[c], p[*j] - p[c]) * sign >= -1e-12
            }) {
                continue;
            }
            out.push([ids[a], ids[b], ids[c]]);
            ring.remove(i);
            found = true;
            break;
        }
        if !found {
            return None;
        }
    }
    out.push([ids[ring[0]], ids[ring[1]], ids[ring[2]]]);
    Some(out)
}

pub(super) fn read_model(
    path: &std::path::Path,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<u8>, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > MAX_FILE_BYTES {
        return Err("El modelo supera 512 MB.".into());
    }
    let mut bytes = Vec::new();
    let mut chunk = [0; 64 * 1024];
    loop {
        if cancel() {
            return Err("Cálculo cancelado.".into());
        }
        let n = file.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        if bytes.len() as u64 + n as u64 > MAX_FILE_BYTES {
            return Err("El modelo supera 512 MB.".into());
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
    Ok(bytes)
}
