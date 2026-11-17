//! Crestas y valles independientes de la vista. Las aristas vivas (ángulo diedro
//! grande) se dibujan tal cual; en las zonas suaves se buscan las líneas de cresta
//! de la curvatura principal dominante (donde su derivada a lo largo de su propia
//! dirección cruza cero con un máximo) y se encadenan en trazos largos. Las cadenas
//! débiles —poca curvatura acumulada a lo largo del trazo— se descartan: es lo que
//! separa el relieve de una escultura de su ruido.
use super::*;
use std::collections::{HashMap, VecDeque};

type Key = (usize, usize);
struct Piece {
    keys: [Option<Key>; 2],
    points: [V3; 2],
    normals: [V3; 2],
    strength: f64,
}

/// Ángulo diedro a partir del cual una arista es viva.
const CREASE_DEG: f64 = 35.;
/// Longitud mínima de un grupo de aristas vivas, como fracción de la diagonal.
const MIN_CREASE: f64 = 0.04;

pub(super) fn draw(
    shape: &Shape,
    mesh: &Mesh,
    threshold: f64,
    cancel: &dyn Fn() -> bool,
) -> Result<[Vec<Vec<V3>>; 2], String> {
    let points = &shape.points;
    let n = points.len();
    let mut out: [Vec<Vec<V3>>; 2] = [vec![], vec![]];
    let mut sharp = vec![false; n];
    let face_normals: Vec<Option<V3>> = mesh
        .triangles
        .iter()
        .map(|t| {
            let p = t.map(|i| points[i]);
            let c = (p[1] - p[0]).cross(p[2] - p[0]);
            (c.length() > shape.size * shape.size * 1e-14).then(|| c.normalized())
        })
        .collect();
    let mut faces = HashMap::<Key, Vec<usize>>::new();
    for (f, t) in mesh.triangles.iter().enumerate() {
        for j in 0..3 {
            let (a, b) = (t[j], t[(j + 1) % 3]);
            faces.entry((a.min(b), a.max(b))).or_default().push(f);
        }
    }
    let limit = CREASE_DEG.to_radians().cos();
    let mut creases: [Vec<(usize, usize)>; 2] = [vec![], vec![]];
    let mut edges: Vec<_> = faces.into_iter().collect();
    edges.sort_unstable_by_key(|(k, _)| *k);
    for (i, ((a, b), fs)) in edges.iter().enumerate() {
        if i % 1024 == 0 && cancel() {
            return Err("Cálculo cancelado.".into());
        }
        let [f0, f1] = fs[..] else { continue };
        let (Some(n0), Some(n1)) = (face_normals[f0], face_normals[f1]) else {
            continue;
        };
        if n0.dot(n1) > limit {
            continue;
        }
        let Some(other) = mesh.triangles[f1]
            .iter()
            .copied()
            .find(|v| v != a && v != b)
        else {
            continue;
        };
        let convex = (points[other] - points[*a]).dot(n0) < 0.;
        sharp[*a] = true;
        sharp[*b] = true;
        creases[usize::from(!convex)].push((*a, *b));
    }
    // La simplificación de una escultura deja aristas vivas sueltas y cortas:
    // solo se dibujan los grupos conectados (crestas y valles juntos, como las
    // aristas de una peana) con cierta longitud respecto al modelo.
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for &(a, b) in creases.iter().flatten() {
        let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
        parent[ra] = rb;
    }
    let mut length = vec![0.; n];
    for &(a, b) in creases.iter().flatten() {
        let r = root(&mut parent, a);
        length[r] += (points[a] - points[b]).length();
    }
    let min_crease = shape.size * MIN_CREASE;
    for (side, list) in creases.into_iter().enumerate() {
        let kept = list
            .into_iter()
            .filter(|&(a, _)| length[root(&mut parent, a)] >= min_crease)
            .map(|(a, b)| [points[a], points[b]])
            .collect();
        out[side].extend(join(kept));
    }

    // Curvaturas y direcciones principales de cada vértice.
    let mut fields = [
        (vec![0.; n], vec![V3::default(); n], vec![false; n]),
        (vec![0.; n], vec![V3::default(); n], vec![false; n]),
    ];
    let mut magnitudes = Vec::with_capacity(n);
    for (i, (&[a, b, c], &[tx, ty])) in shape.tensors.iter().zip(&shape.frames).enumerate() {
        let mid = (a + c) * 0.5;
        let r = (((a - c) * 0.5).powi(2) + b * b).sqrt();
        let (k1, k2) = (mid + r, mid - r);
        let (s, co) = (0.5 * (2. * b).atan2(a - c)).sin_cos();
        let (e1, e2) = (tx * co + ty * s, ty * co - tx * s);
        let usable = !shape.boundary[i] && !sharp[i];
        if usable {
            magnitudes.push(k1.abs().max(k2.abs()));
        }
        fields[0].0[i] = k1;
        fields[0].1[i] = e1;
        fields[0].2[i] = usable && k1 > 0. && k1 > -k2;
        fields[1].0[i] = -k2;
        fields[1].1[i] = e2;
        fields[1].2[i] = usable && k2 < 0. && -k2 > k1;
    }
    if magnitudes.is_empty() {
        return Ok(out);
    }
    magnitudes.sort_unstable_by(f64::total_cmp);
    let scale = magnitudes[((magnitudes.len() - 1) as f64 * 0.98) as usize].max(1e-12);
    let gate = threshold * scale;
    let (length, count) = mesh.triangles.iter().fold((0., 0usize), |(l, c), t| {
        (l + (points[t[0]] - points[t[1]]).length(), c + 1)
    });
    let lift = 0.25 * length / count.max(1) as f64;
    // Curvatura acumulada (en radianes) que debe reunir un trazo para dibujarse.
    let min_strength = 0.3 + threshold;
    for (side, (k, dirs, ok)) in fields.iter().enumerate() {
        let gradient = shape.gradient(k, Some(ok), cancel)?;
        let derivative: Vec<f64> = gradient.iter().zip(dirs).map(|(g, e)| g.dot(*e)).collect();
        let mut pieces = Vec::new();
        for (index, t) in mesh.triangles.iter().enumerate() {
            if index % 256 == 0 && cancel() {
                return Err("Cálculo cancelado.".into());
            }
            if !t.iter().all(|i| ok[*i]) {
                continue;
            }
            let p = t.map(|i| points[i]);
            let dir = dirs[t[0]];
            let flip = t.map(|i| if dir.dot(dirs[i]) < 0. { -1. } else { 1. });
            let values = [0, 1, 2].map(|j| derivative[t[j]] * flip[j]);
            // Solo máximos: la derivada alineada debe decrecer a lo largo de la dirección.
            let e = p[1] - p[0];
            let f = p[2] - p[0];
            let normal = e.cross(f);
            let grad = (f.cross(normal) * (values[1] - values[0])
                + normal.cross(e) * (values[2] - values[0]))
                * (1. / normal.dot(normal).max(1e-30));
            let along = (0..3).fold(V3::default(), |s, j| s + dirs[t[j]] * flip[j]);
            if grad.dot(along) >= 0. {
                continue;
            }
            let mut cuts: Vec<(V3, [f64; 3], Key)> = Vec::new();
            for j in 0..3 {
                let m = (j + 1) % 3;
                let (a, b) = (values[j], values[m]);
                if (a < 0.) == (b < 0.) || (a - b).abs() < 1e-30 {
                    continue;
                }
                let u = a / (a - b);
                let mut bary = [0.; 3];
                bary[j] = 1. - u;
                bary[m] = u;
                cuts.push((
                    p[j] + (p[m] - p[j]) * u,
                    bary,
                    (t[j].min(t[m]), t[j].max(t[m])),
                ));
            }
            if cuts.len() != 2 {
                continue;
            }
            let level = |b: [f64; 3]| (0..3).map(|j| b[j] * (k[t[j]] - gate)).sum::<f64>();
            let (la, lb) = (level(cuts[0].1), level(cuts[1].1));
            if la < 0. && lb < 0. {
                continue;
            }
            let (mut from, mut to) = (0., 1.);
            let mut keys = [Some(cuts[0].2), Some(cuts[1].2)];
            if la < 0. {
                from = la / (la - lb);
                keys[0] = None;
            }
            if lb < 0. {
                to = la / (la - lb);
                keys[1] = None;
            }
            let a = cuts[0].0;
            let d = cuts[1].0 - a;
            let segment = [a + d * from, a + d * to];
            let normal =
                |b: [f64; 3]| (0..3).fold(V3::default(), |s, j| s + shape.normals[t[j]] * b[j]);
            let (na, nb) = (normal(cuts[0].1), normal(cuts[1].1));
            let normals = [na + (nb - na) * from, na + (nb - na) * to].map(|n| n.normalized());
            let mean = (k[t[0]] + k[t[1]] + k[t[2]]) / 3.;
            pieces.push(Piece {
                keys,
                points: segment,
                normals,
                strength: (segment[1] - segment[0]).length() * mean,
            });
        }
        out[side].extend(chain(&pieces, min_strength, lift));
    }
    Ok(out)
}

/// Encadena tramos que comparten la arista de malla donde cortan; se detiene en
/// los cruces para no fundir dos crestas distintas.
fn chain(pieces: &[Piece], min_strength: f64, lift: f64) -> Vec<Vec<V3>> {
    let mut at = HashMap::<Key, Vec<usize>>::new();
    for (i, p) in pieces.iter().enumerate() {
        for k in p.keys.iter().flatten() {
            at.entry(*k).or_default().push(i);
        }
    }
    let mut used = vec![false; pieces.len()];
    let mut out = Vec::new();
    for start in 0..pieces.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let first = &pieces[start];
        let mut line = VecDeque::from(vec![
            (first.points[0], first.normals[0]),
            (first.points[1], first.normals[1]),
        ]);
        let mut strength = pieces[start].strength;
        for forward in [true, false] {
            let mut key = pieces[start].keys[usize::from(forward)];
            while let Some(k) = key {
                let list = &at[&k];
                if list.len() != 2 {
                    break;
                }
                let Some(&next) = list.iter().find(|i| !used[**i]) else {
                    break;
                };
                used[next] = true;
                let piece = &pieces[next];
                strength += piece.strength;
                let far = usize::from(piece.keys[0] == Some(k));
                let point = (piece.points[far], piece.normals[far]);
                if forward {
                    line.push_back(point);
                } else {
                    line.push_front(point);
                }
                key = piece.keys[far];
            }
        }
        if strength >= min_strength {
            out.push(relax(line.into(), lift));
        }
    }
    out
}

/// Dos pasadas de suavizado laplaciano: quitan el diente de sierra que deja
/// cortar triángulo a triángulo sin mover los extremos. Cada punto solo se
/// desplaza en su plano tangente; si no, el trazo se hundiría bajo una superficie
/// convexa y el cálculo de líneas ocultas lo borraría.
fn relax(line: Vec<(V3, V3)>, lift: f64) -> Vec<V3> {
    let (mut points, normals): (Vec<V3>, Vec<V3>) = line.into_iter().unzip();
    for _ in 0..2 {
        let copy = points.clone();
        for i in 1..points.len().saturating_sub(1) {
            let d = (copy[i - 1] + copy[i + 1]) * 0.25 - copy[i] * 0.5;
            points[i] = copy[i] + d - normals[i] * d.dot(normals[i]);
        }
    }
    // Un poco por encima de la malla para que la ocultación no lo corte a tramos.
    for (p, n) in points.iter_mut().zip(&normals) {
        *p = *p + *n * lift;
    }
    points
}

/// Une aristas vivas contiguas en polilíneas.
fn join(segments: Vec<[V3; 2]>) -> Vec<Vec<V3>> {
    let key = |p: V3| {
        (
            (p.x * 1e7).round() as i64,
            (p.y * 1e7).round() as i64,
            (p.z * 1e7).round() as i64,
        )
    };
    let mut at = HashMap::<(i64, i64, i64), Vec<usize>>::new();
    for (i, s) in segments.iter().enumerate() {
        for p in s {
            at.entry(key(*p)).or_default().push(i);
        }
    }
    let mut used = vec![false; segments.len()];
    let mut out = Vec::new();
    for start in 0..segments.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut line = VecDeque::from(segments[start].to_vec());
        for forward in [true, false] {
            loop {
                let end = if forward {
                    *line.back().unwrap()
                } else {
                    line[0]
                };
                let list = &at[&key(end)];
                if list.len() != 2 {
                    break;
                }
                let Some(&next) = list.iter().find(|i| !used[**i]) else {
                    break;
                };
                used[next] = true;
                let s = segments[next];
                let far = if key(s[0]) == key(end) { s[1] } else { s[0] };
                if forward {
                    line.push_back(far);
                } else {
                    line.push_front(far);
                }
            }
        }
        out.push(line.into());
    }
    out
}
