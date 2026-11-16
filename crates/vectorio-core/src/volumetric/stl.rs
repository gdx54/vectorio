//! STL binario y ASCII sin dependencias. La cabecera «solid» por sí sola no
//! distingue formatos: primero se contrasta el tamaño binario declarado.
use super::*;
use std::path::Path;

pub fn load_mesh(path: &Path) -> Result<(Mesh, MeshReport), String> {
    load_mesh_cancellable(path, &|| false)
}
pub fn load_mesh_cancellable(
    path: &Path,
    cancel: &dyn Fn() -> bool,
) -> Result<(Mesh, MeshReport), String> {
    let bytes = super::import::read_model(path, cancel)?;
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Modelo");
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("stl"))
    {
        parse_stl_cancellable(&bytes, name, cancel)
    } else {
        super::import::parse_obj_cancellable(
            std::str::from_utf8(&bytes).map_err(|_| "El OBJ no es texto UTF-8 válido.")?,
            name,
            cancel,
        )
    }
}
pub fn load_stl(path: &Path) -> Result<(Mesh, MeshReport), String> {
    let bytes = super::import::read_model(path, &|| false)?;
    parse_stl(
        &bytes,
        path.file_stem().and_then(|s| s.to_str()).unwrap_or("STL"),
    )
}
pub fn parse_stl(bytes: &[u8], name: &str) -> Result<(Mesh, MeshReport), String> {
    parse_stl_cancellable(bytes, name, &|| false)
}
fn parse_stl_cancellable(
    bytes: &[u8],
    name: &str,
    cancel: &dyn Fn() -> bool,
) -> Result<(Mesh, MeshReport), String> {
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err("STL demasiado grande: máximo 512 MB.".into());
    }
    if bytes.len() >= 84 {
        let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
        let expected = count.checked_mul(50).and_then(|n| n.checked_add(84));
        if expected == Some(bytes.len()) {
            if count > MAX_FACES {
                return Err("STL supera 5 millones de triángulos.".into());
            }
            return binary(bytes, name, count, cancel);
        }
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "STL binario truncado o con un número de triángulos incorrecto.")?;
    if !text
        .trim_start_matches('\u{feff}')
        .trim_start()
        .get(..5)
        .is_some_and(|s| s.eq_ignore_ascii_case("solid"))
    {
        return Err(
            "STL no válido: se esperaba una cabecera binaria completa o «solid» en ASCII.".into(),
        );
    }
    ascii(text.trim_start_matches('\u{feff}'), name, cancel)
}
fn empty(name: &str) -> Mesh {
    Mesh {
        name: name.into(),
        vertices: vec![],
        triangles: vec![],
        objects: vec![],
        face_objects: vec![],
    }
}
fn binary(
    bytes: &[u8],
    name: &str,
    count: usize,
    cancel: &dyn Fn() -> bool,
) -> Result<(Mesh, MeshReport), String> {
    let mut mesh = empty(name);
    mesh.objects.push(name.into());
    for face in 0..count {
        if face % 4096 == 0 && cancel() {
            return Err("Cálculo cancelado.".into());
        }
        let at = 84 + face * 50;
        let start = mesh.vertices.len();
        // Normales y atributo de color STL no se convierten en tintas. El winding
        // de los vértices se conserva; la oclusión utiliza caras de doble lado.
        for vertex in 0..3 {
            let mut p = [0.; 3];
            for (axis, value) in p.iter_mut().enumerate() {
                let i = at + 12 + vertex * 12 + axis * 4;
                *value = f32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as f64;
            }
            mesh.vertices.push(p);
        }
        mesh.triangles.push([start, start + 1, start + 2]);
        mesh.face_objects.push(0);
    }
    mesh.validated_cancellable(cancel)
}
fn ascii(text: &str, name: &str, cancel: &dyn Fn() -> bool) -> Result<(Mesh, MeshReport), String> {
    let mut mesh = empty(name);
    let mut state = 0;
    let mut active = false;
    let mut owner = 0;
    let mut vertices = 0;
    for (line_no, line) in text.lines().enumerate() {
        if line_no % 4096 == 0 && cancel() {
            return Err("Cálculo cancelado.".into());
        }
        if line.len() > 65_536 {
            return Err(format!("STL: línea {} demasiado larga.", line_no + 1));
        }
        let words: Vec<_> = line.split_ascii_whitespace().collect();
        if words.is_empty() {
            continue;
        }
        let bad = || {
            format!(
                "STL ASCII: estructura no válida en la línea {}.",
                line_no + 1
            )
        };
        let keyword = words[0];
        match state {
            0 if keyword.eq_ignore_ascii_case("solid") && !active => {
                if mesh.objects.len() >= MAX_FACES {
                    return Err("Demasiados sólidos en el STL.".into());
                }
                let label = words[1..].join(" ");
                if label.len() > 4096 {
                    return Err(bad());
                }
                owner = mesh.objects.len();
                mesh.objects
                    .push(if label.is_empty() { name.into() } else { label });
                active = true;
            }
            0 if keyword.eq_ignore_ascii_case("endsolid") && active => {
                active = false;
            }
            0 if keyword.eq_ignore_ascii_case("facet") && active => {
                if words.len() != 5 || !words[1].eq_ignore_ascii_case("normal") {
                    return Err(bad());
                }
                for n in &words[2..] {
                    if n.parse::<f64>().map_or(true, |n| !n.is_finite()) {
                        return Err(bad());
                    }
                }
                if mesh.triangles.len() >= MAX_FACES {
                    return Err("STL supera 5 millones de triángulos.".into());
                }
                state = 1;
                vertices = 0;
            }
            1 if words.len() == 2
                && keyword.eq_ignore_ascii_case("outer")
                && words[1].eq_ignore_ascii_case("loop") =>
            {
                state = 2
            }
            2 if words.len() == 4 && keyword.eq_ignore_ascii_case("vertex") => {
                let mut p = [0.; 3];
                for (v, n) in p.iter_mut().zip(&words[1..]) {
                    *v = n.parse().map_err(|_| bad())?;
                }
                mesh.vertices.push(p);
                vertices += 1;
                if vertices == 3 {
                    state = 3;
                }
            }
            3 if words.len() == 1 && keyword.eq_ignore_ascii_case("endloop") => state = 4,
            4 if words.len() == 1 && keyword.eq_ignore_ascii_case("endfacet") => {
                let n = mesh.vertices.len();
                mesh.triangles.push([n - 3, n - 2, n - 1]);
                mesh.face_objects.push(owner);
                state = 0;
            }
            _ => return Err(bad()),
        }
    }
    if state != 0 || active {
        return Err("STL ASCII incompleto: faltan endfacet o endsolid.".into());
    }
    mesh.validated_cancellable(cancel)
}
