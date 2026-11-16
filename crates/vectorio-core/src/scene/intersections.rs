//! Encuentros entre caras en coordenadas de hoja y profundidad, sin booleanas 3D.
use std::collections::BTreeMap;
use super::{Scene, Stroke, PartId, MIN_STROKE_MM};
use super::hidden::{Face, Grid, Vertex};
use crate::geom::{p, P};
#[cfg(test)] use crate::geom::BBox;

fn overlaps(a: &Face, b: &Face) -> bool {
    a.bbox.min.x <= b.bbox.max.x && a.bbox.max.x >= b.bbox.min.x &&
        a.bbox.min.y <= b.bbox.max.y && a.bbox.max.y >= b.bbox.min.y &&
        a.min_depth <= b.max_depth + 1e-10 && a.max_depth + 1e-10 >= b.min_depth
}
fn segment(a: &Face, b: &Face, reach: f64) -> Option<[Vertex; 2]> {
    let g = a.gradient - b.gradient;
    let norm = g.len();
    if norm < 1e-12 { return None; }
    let center = a.bbox.center().lerp(b.bbox.center(), 0.5);
    let q = center - g * ((a.depth(center) - b.depth(center)) / g.dot(g));
    let d = p(-g.y, g.x) * (reach / norm);
    let (start, end) = (q - d, q + d);
    let (lo, hi) = a.clip(start, end, 0.0)?;
    let (u, v) = b.clip(start, end, 0.0)?;
    let (lo, hi) = (lo.max(u), hi.min(v));
    if hi <= lo { return None; }
    let (x, y) = (start.lerp(end, lo), start.lerp(end, hi));
    if x.dist(y) < MIN_STROKE_MM { return None; }
    // Si todo el encuentro coincide con un borde, ya lo aporta el propio sólido.
    // Evita además duplicar contactos entre partes distintas (terreno y edificio).
    if [a,b].iter().any(|face|face.drawn_boundaries.iter().any(|&(u,v)|{
        let d=v-u;let len=d.len();if len<1e-12{return false;}
        let unit=d*(1.0/len);
        [x,y].iter().all(|q|super::cross(unit,*q-u).abs()<1e-8 && (*q-u).dot(unit)>=-1e-8 && (*q-u).dot(unit)<=len+1e-8)
    })){return None;}
    Some([Vertex { point: x, depth: a.depth(x) }, Vertex { point: y, depth: a.depth(y) }])
}
fn meets(a: Vertex, b: Vertex) -> bool { a.point.dist(b.point) <= 1e-7 && (a.depth - b.depth).abs() <= 1e-8 }
fn chains(mut edges: Vec<[Vertex; 2]>, part: PartId, out: &mut Vec<Stroke>) {
    // El conjunto de cada pareja de sólidos es pequeño. Une solo extremos iguales,
    // sin reordenar el resto del dibujo ni acercar trazos que no se tocan.
    while let Some(edge) = edges.pop() {
        let mut line = edge.to_vec();
        loop {
            let mut found = None;
            for (i,e) in edges.iter().enumerate() {
                if meets(*line.last().unwrap(),e[0]) { found=Some((i,e[1],false)); break; }
                if meets(*line.last().unwrap(),e[1]) { found=Some((i,e[0],false)); break; }
                if meets(line[0],e[1]) { found=Some((i,e[0],true)); break; }
                if meets(line[0],e[0]) { found=Some((i,e[1],true)); break; }
            }
            let Some((i,point,prepend))=found else { break };
            edges.remove(i); if prepend { line.insert(0,point); } else { line.push(point); }
        }
        if meets(line[0],*line.last().unwrap()) { let last=line.len()-1;line[last]=line[0]; }
        out.push(Stroke { vertices: line, part, whole: false, fill: false });
    }
}
pub(super) fn strokes(scene: &Scene, grid: &Grid) -> Vec<Stroke> {
    let mut groups: BTreeMap<(u64,u64,PartId),Vec<[Vertex;2]>> = BTreeMap::new();
    let mut stamps = vec![usize::MAX;scene.faces.len()];
    let reach = scene.paper.width().hypot(scene.paper.height()) * 2.0;
    for (i,a) in scene.faces.iter().enumerate() {
        let Some((owner,part))=a.owner else {continue};
        let Some((x0,y0,x1,y1))=grid.range(a.bbox) else {continue};
        for y in y0..=y1 {for x in x0..=x1 {for &j in &grid.cells[y*grid.cols+x] {
            if j<=i || stamps[j]==i {continue;} stamps[j]=i;
            let b=&scene.faces[j];let Some((other,other_part))=b.owner else {continue};
            if owner==other || !overlaps(a,b) {continue;}
            if let Some(edge)=segment(a,b,reach) {
                let part=if owner>other {part}else{other_part};
                groups.entry((owner.min(other),owner.max(other),part)).or_default().push(edge);
            }
        }}}
    }
    let mut out=Vec::new();for ((_,_,part),edges) in groups {chains(edges,part,&mut out);}out
}

// Área de intersección de polígonos convexos coplanares. Solo se usa para el
// diagnóstico del kit; no participa en el dibujo ni introduce recortes raster.
fn overlap_area(a:&Face, mut poly:Vec<P>)->f64 {
    for (origin,normal) in &a.planes {
        if poly.is_empty(){return 0.0;}
        let mut next=Vec::new();
        for (&x,&y) in poly.iter().zip(poly.iter().cycle().skip(1)).take(poly.len()) {
            let (u,v)=((x-*origin).dot(*normal),(y-*origin).dot(*normal));
            if u>=0.0{next.push(x);}
            if (u>=0.0)!=(v>=0.0){next.push(x.lerp(y,u/(u-v)));}
        }poly=next;
    }
    crate::geom::signed_area(&poly).abs()
}
impl Scene {
    /// Fracción de la pared vertical visible de una parte que queda en luz (sin
    /// trama en el grabado). Muestrea la hoja cada `step_mm` y toma en cada punto
    /// la cara más cercana a la cámara.
    #[cfg(test)]
    pub(crate) fn lit_wall_fraction(&self, part:&str, step_mm:f64)->f64 {
        let Some(id)=self.names.iter().position(|n|n==part) else{return 0.0};
        let grid=Grid::new(self.paper,&self.faces);
        let (mut lit,mut walls)=(0usize,0usize);
        let mut y=self.paper.min.y+step_mm*0.5;
        while y<self.paper.max.y {
            let mut x=self.paper.min.x+step_mm*0.5;
            while x<self.paper.max.x {
                let q=p(x,y);
                let front=self.front_face(&grid,q);
                x+=step_mm;
                let Some(face)=front else{continue};
                let owned=face.owner.is_some_and(|(_,owner)|owner.0 as usize==id);
                if !owned||face.normal.z.abs()>1e-6 {continue;}
                walls+=1;
                if self.tone(face.normal)==super::Tone::Lit {lit+=1;}
            }
            y+=step_mm;
        }
        if walls==0 {0.0} else {lit as f64/walls as f64}
    }
    /// Cara sólida que la cámara ve primero en el punto `q` de la hoja.
    #[cfg(test)]
    fn front_face(&self, grid:&Grid, q:P)->Option<&Face> {
        let tiny=BBox{min:q,max:q};
        let (x0,y0,_,_)=grid.range(tiny)?;
        let inside=|face:&&Face|face.owner.is_some()&&self.camera.facing(face.normal)&&
            face.planes.iter().all(|(origin,normal)|(q-*origin).dot(*normal)>=0.0);
        grid.cells[y0*grid.cols+x0].iter().map(|&i|&self.faces[i]).filter(inside)
            .min_by(|a,b|a.depth(q).total_cmp(&b.depth(q)))
    }
    /// Diagnóstico del kit: caras paralelas con huella común, separadas menos de
    /// mínimo_mm, o coplanares y solapadas con la misma normal exterior.
    /// Un contacto con normales opuestas es una unión limpia y no se señala.
    pub fn construction_issues(&self, minimum_mm:f64)->Vec<String> {
        if !minimum_mm.is_finite() || minimum_mm<=0.0{return Vec::new();}
        // Solo se comparan caras casi paralelas, de sólidos distintos, cuyos planos
        // quedan a menos del umbral y cuyas cajas en la hoja se tocan. Agrupar por
        // normal y ordenar por plano evita comparar todas con todas.
        let faces:Vec<(usize,&Face)>=self.faces.iter().enumerate().filter(|(_,f)|f.owner.is_some()).collect();
        let mut reps:Vec<super::V3>=Vec::new();let mut group=Vec::with_capacity(faces.len());
        for (_,f) in &faces{
            let g=reps.iter().position(|r|r.dot(f.normal)>=1.0-1e-9).unwrap_or_else(||{reps.push(f.normal);reps.len()-1});
            group.push(g);
        }
        // Dos caras emparejables pueden caer en grupos vecinos si están en el borde de uno.
        let near:Vec<Vec<usize>>=(0..reps.len()).map(|i|(0..reps.len()).filter(|&j|reps[i].dot(reps[j])>=1.0-1e-7).collect()).collect();
        let mut members=vec![Vec::new();reps.len()];
        for (k,&g) in group.iter().enumerate(){members[g].push(k);}
        let reach=minimum_mm/self.camera.scale;
        let extent=faces.iter().map(|(_,f)|f.world_origin.length()).fold(0.0,f64::max);
        let slack=reach+extent*1e-3+1e-9;
        let mut pairs=Vec::new();
        for (g,list) in members.iter().enumerate(){
            let r=reps[g];
            let mut pool:Vec<(f64,usize)>=near[g].iter().flat_map(|&h|members[h].iter()).map(|&k|(faces[k].1.world_origin.dot(r),k)).collect();
            pool.sort_by(|a,b|a.0.total_cmp(&b.0));
            for &k in list{
                let (i,a)=faces[k];let d=a.world_origin.dot(r);
                let start=pool.partition_point(|(e,_)|*e<d-slack);
                for &(e,m) in &pool[start..]{
                    if e>d+slack{break;}
                    let (j,b)=faces[m];
                    if j<=i||a.owner.map(|o|o.0)==b.owner.map(|o|o.0)||a.normal.dot(b.normal)<1.0-1e-10{continue;}
                    let grow=minimum_mm;
                    if a.bbox.min.x>b.bbox.max.x+grow||b.bbox.min.x>a.bbox.max.x+grow||a.bbox.min.y>b.bbox.max.y+grow||b.bbox.min.y>a.bbox.max.y+grow{continue;}
                    pairs.push((i,j));
                }
            }
        }
        pairs.sort_unstable();pairs.dedup();
        let mut issues=Vec::new();
        for (i,j) in pairs{
            let (a,b)=(&self.faces[i],&self.faces[j]);
            let ((owner,_),(other,_))=(a.owner.unwrap(),b.owner.unwrap());
            let gap=(a.world_origin-b.world_origin).dot(a.normal);
            let mm=gap.abs()*self.camera.scale;
            if mm>=minimum_mm {continue;}
            let projected:Vec<_>=b.points.iter().map(|q| {
                self.camera.project(self.camera.unproject(*q,b.depth(*q))+a.normal*gap).0
            }).collect();
            if overlap_area(a,projected)>1e-5 {
                issues.push(if mm<1e-6 {format!("Caras coplanares solapadas: {owner}/{other}")}
                    else{format!("Rebanada de {mm:.3} mm: {owner}/{other}, normal {:?}, {:?} / {:?}",a.normal,a.points.iter().map(|q|self.camera.unproject(*q,a.depth(*q))).collect::<Vec<_>>(),b.points.iter().map(|q|self.camera.unproject(*q,b.depth(*q))).collect::<Vec<_>>())});
            }
        }issues
    }
}
