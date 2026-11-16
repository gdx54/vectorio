//! Cámara de Volumen. Sus ajustes y encuadre son independientes de los generadores.
//! El adaptador hacia scene comparte únicamente la proyección y oclusión analíticas.
use super::V3;
use crate::{BBox, P};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub yaw_deg: f64,
    pub elev_deg: f64,
    pub scale: f64,
    pub world_center: V3,
    pub paper_center: P,
    pub perspective: Option<[f64; 2]>,
}
impl Default for Camera {
    fn default() -> Self {
        crate::scene::Camera::default().into()
    }
}
impl From<crate::scene::Camera> for Camera {
    fn from(c: crate::scene::Camera) -> Self {
        Self {
            yaw_deg: c.yaw_deg,
            elev_deg: c.elev_deg,
            scale: c.scale,
            world_center: c.world_center,
            paper_center: c.paper_center,
            perspective: c.perspective,
        }
    }
}
impl From<Camera> for crate::scene::Camera {
    fn from(c: Camera) -> Self {
        Self {
            yaw_deg: c.yaw_deg,
            elev_deg: c.elev_deg,
            scale: c.scale,
            world_center: c.world_center,
            paper_center: c.paper_center,
            perspective: c.perspective,
        }
    }
}
impl Camera {
    fn analytic(self) -> crate::scene::Camera {
        self.into()
    }
    pub fn axes(&self) -> (V3, V3, V3) {
        self.analytic().axes()
    }
    pub fn project(&self, q: V3) -> (P, f64) {
        self.analytic().project(q)
    }
    pub fn unproject(&self, q: P, z: f64) -> V3 {
        self.analytic().unproject(q, z)
    }
    pub fn facing(&self, n: V3) -> bool {
        self.analytic().facing(n)
    }
    pub fn facing_at(&self, n: V3, q: V3) -> bool {
        self.analytic().facing_at(n, q)
    }
    pub fn ground(&self, q: P, z: f64) -> V3 {
        self.analytic().ground(q, z)
    }
    pub fn near_value(&self, q: V3) -> f64 {
        self.analytic().near_value(q)
    }
    pub fn clip_segment(&self, a: V3, b: V3) -> Option<[V3; 2]> {
        self.analytic().clip_segment(a, b)
    }
    pub fn clip_polygon(&self, p: &[V3]) -> Vec<V3> {
        self.analytic().clip_polygon(p)
    }
    pub fn weights(&self, w: [f64; 3], points: [V3; 3]) -> [f64; 3] {
        self.analytic().weights(w, points)
    }
    pub fn fit(&mut self, world: [V3; 2], paper: BBox, margin: f64) {
        let mut camera = self.analytic();
        camera.fit(world, paper, margin);
        *self = camera.into();
    }
}
