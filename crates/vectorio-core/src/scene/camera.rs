use std::ops::{Add, Mul, Sub};
use crate::geom::{p, BBox, P};

/// Coordenada del mundo en metros; z crece hacia arriba.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V3 { pub x: f64, pub y: f64, pub z: f64 }

impl V3 {
    pub fn finite(self) -> bool { self.x.is_finite() && self.y.is_finite() && self.z.is_finite() }
    pub const fn new(x: f64, y: f64, z: f64) -> Self { Self { x, y, z } }
    pub fn dot(self, other: Self) -> f64 { self.x * other.x + self.y * other.y + self.z * other.z }
    pub fn cross(self, o: Self) -> Self {
        Self::new(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }
    pub fn length(self) -> f64 { self.dot(self).sqrt() }
    /// Un vector nulo conserva su valor; no introduce NaN en caras degeneradas.
    pub fn normalized(self) -> Self { let l = self.length(); if l > 1e-15 { self * (1.0 / l) } else { Self::default() } }
}
impl Add for V3 { type Output = Self; fn add(self, o: Self) -> Self { Self::new(self.x + o.x, self.y + o.y, self.z + o.z) } }
impl Sub for V3 { type Output = Self; fn sub(self, o: Self) -> Self { Self::new(self.x - o.x, self.y - o.y, self.z - o.z) } }
impl Mul<f64> for V3 { type Output = Self; fn mul(self, k: f64) -> Self { Self::new(self.x * k, self.y * k, self.z * k) } }

/// Cámara ortográfica. El yaw sitúa al observador en (cos yaw, sin yaw).
/// La elevación está entre 0 y 90 grados, sin incluir los extremos.
/// La profundidad aumenta al alejarse del observador y se mide en metros.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub yaw_deg: f64,
    pub elev_deg: f64,
    pub scale: f64,
    pub world_center: V3,
    pub paper_center: P,
    /// Distancia focal y plano cercano en unidades del mundo; None conserva la ortográfica.
    pub perspective: Option<[f64; 2]>,
}

impl Default for Camera {
    fn default() -> Self {
        Self { yaw_deg: 45.0, elev_deg: (1.0 / 2f64.sqrt()).atan().to_degrees(), scale: 10.0,
            perspective: None, world_center: V3::default(), paper_center: p(105.0, 148.5) }
    }
}

impl Camera {
    pub(super) fn valid(&self) -> bool {
        self.yaw_deg.is_finite() && self.elev_deg > 0.0 && self.elev_deg < 90.0 && self.scale.is_finite() && self.scale > 0.0 &&
            self.perspective.is_none_or(|[d,n]|d.is_finite()&&n.is_finite()&&d>0.&&n>0.) && self.world_center.finite() && self.paper_center.x.is_finite() && self.paper_center.y.is_finite()
    }
    // Base ortonormal: derecha, abajo y lejos. Evita fórmulas distintas para la inversa.
    pub(super) fn basis(&self) -> (V3, V3, V3) {
        let (sy, cy) = self.yaw_deg.to_radians().sin_cos();
        let (se, ce) = self.elev_deg.to_radians().sin_cos();
        (V3::new(-sy, cy, 0.0), V3::new(se * cy, se * sy, -ce), V3::new(-ce * cy, -ce * sy, -se))
    }
    /// Ejes del mundo que van a la derecha y hacia abajo en la hoja, y hacia el fondo.
    pub fn axes(&self) -> (V3, V3, V3) { self.basis() }
    pub fn project(&self, point: V3) -> (P, f64) {
        if !self.valid() || !point.finite() { return (p(0.0, 0.0), 0.0); }
        let (right, down, away) = self.basis();
        let q = point - self.world_center;
        let z=q.dot(away);
        if let Some([d, near])=self.perspective {
            let w=(d+z).max(near*1e-6);
            return (self.paper_center+p(q.dot(right),q.dot(down))*(self.scale*d/w),d-d*d/w);
        }
        (self.paper_center + p(q.dot(right), q.dot(down)) * self.scale, z)
    }
    pub fn facing(&self, normal: V3) -> bool { normal.dot(self.basis().2) < -1e-12 }
    pub fn unproject(&self, point: P, depth: f64) -> V3 {
        let (right, down, away) = self.basis();
        let mut q = (point - self.paper_center) * (1.0 / self.scale);
        let depth=if let Some([d,_])=self.perspective {let z=depth/(1.-depth/d);q=q*((d+z)/d);z}else{depth};
        self.world_center + right * q.x + down * q.y + away * depth
    }
    pub fn facing_at(&self, normal:V3, point:V3)->bool {
        if let Some([d,_])=self.perspective { normal.dot(self.world_center-self.basis().2*d-point)>1e-12 }
        else {self.facing(normal)}
    }
    pub fn near_value(&self,q:V3)->f64 {self.perspective.map_or(1.,|[d,n]|d+(q-self.world_center).dot(self.basis().2)-n)}
    /// Recorte de polígonos convexos antes de dividir por la profundidad.
    pub fn clip_polygon(&self,points:&[V3])->Vec<V3> {
        if self.perspective.is_none(){return points.to_vec();}
        let mut out=Vec::new();
        for (&a,&b) in points.iter().zip(points.iter().cycle().skip(1)).take(points.len()) {
            let (va,vb)=(self.near_value(a),self.near_value(b));
            if va>=0.{out.push(a);}
            if (va>=0.)!=(vb>=0.) {out.push(a+(b-a)*(va/(va-vb)));}
        }
        out
    }
    pub fn clip_segment(&self,a:V3,b:V3)->Option<[V3;2]> {
        let (va,vb)=(self.near_value(a),self.near_value(b));
        if va<0.&&vb<0.{return None;}let mut edge=[a,b];
        if (va>=0.)!=(vb>=0.){edge[usize::from(va>=0.)]=a+(b-a)*(va/(va-vb));}Some(edge)
    }
    /// Pesos baricéntricos corregidos para atributos en perspectiva.
    pub fn weights(&self,mut w:[f64;3],points:[V3;3])->[f64;3]{
        if let Some([d,_])=self.perspective {for i in 0..3{w[i]/=d+(points[i]-self.world_center).dot(self.basis().2);}let sum:f64=w.iter().sum();if sum.abs()>1e-15{for v in &mut w{*v/=sum;}}}w
    }
    /// Intersección del rayo de pantalla con el plano horizontal de altura z.
    pub fn ground(&self, point: P, z: f64) -> V3 {
        if !self.valid() || !point.x.is_finite() || !point.y.is_finite() || !z.is_finite() { return V3::default(); }
        let q = self.unproject(point, 0.0);
        let direction=if let Some([d,_])=self.perspective {q-(self.world_center-self.basis().2*d)}else{self.basis().2};
        if direction.z.abs()<1e-12{return q;}q + direction * ((z - q.z) / direction.z)
    }
    /// Ajusta los ocho vértices de una caja [mínimo, máximo] dentro del área en mm.
    /// Conserva los ángulos. El margen se aplica a los cuatro lados.
    pub fn fit(&mut self, world: [V3; 2], paper: BBox, margin: f64) {
        if !self.valid() || !margin.is_finite() || margin < 0.0 || !super::valid_area(paper) ||
            paper.width() <= 2.0 * margin || paper.height() <= 2.0 * margin || !world.iter().all(|v| v.finite()) ||
            world[0].x > world[1].x || world[0].y > world[1].y || world[0].z > world[1].z { return; }
        self.world_center = (world[0] + world[1]) * 0.5;
        self.paper_center = paper.center();
        self.scale = 1.0;
        let mut bounds = BBox::empty();
        for x in [world[0].x, world[1].x] { for y in [world[0].y, world[1].y] { for z in [world[0].z, world[1].z] {
            bounds.add(self.project(V3::new(x, y, z)).0);
        } } }
        self.scale = ((paper.width() - 2.0 * margin) / bounds.width().max(1e-12))
            .min((paper.height() - 2.0 * margin) / bounds.height().max(1e-12));
    }
}
