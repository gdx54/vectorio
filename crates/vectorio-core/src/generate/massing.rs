//! Masa ortogonal editable: la piel exterior se extrae después de los vaciados.
//! Las caras contiguas se agrupan; solo quedan aristas de giro o de material.
use std::collections::BTreeMap;
use crate::scene::{Scene,V3};
use super::{kit::v,style::{part,Style,Surface}};

pub struct Mass { size:[usize;3], cells:Vec<u8> }
type Point=[usize;3];
type Edge=(Point,Point);
type Runs=BTreeMap<(usize,usize,usize,u8),Vec<(usize,usize)>>;
impl Mass {
    pub fn new(size:[usize;3])->Self {
        assert!(size.iter().all(|n|*n>0&&*n<=96));
        Self{size,cells:vec![0;size.iter().product()]}
    }
    fn index(&self,p:Point)->usize{(p[2]*self.size[1]+p[1])*self.size[0]+p[0]}
    fn get(&self,p:Point)->u8{if (0..3).any(|a|p[a]>=self.size[a]){0}else{self.cells[self.index(p)]}}
    pub fn fill(&mut self,a:Point,b:Point,material:u8){
        for z in a[2]..b[2].min(self.size[2]){for y in a[1]..b[1].min(self.size[1]){for x in a[0]..b[0].min(self.size[0]){
            let i=self.index([x,y,z]);self.cells[i]=material;
        }}}
    }
    /// Extracción en los seis sentidos. Las aristas se cuentan antes de fundir
    /// caras para que las juntas en T no introduzcan trazos sobre un mismo plano.
    pub fn draw(&self,scene:&mut Scene,style:Style,origin:V3,unit:f64)->usize {
        let at=|q:Point|origin+v(q[0] as f64,q[1] as f64,q[2] as f64)*unit;
        let mut edges:BTreeMap<Edge,Vec<(usize,i32,u8)>>=BTreeMap::new();
        let mut count=0;
        for axis in 0..3 {let u=(axis+1)%3;let w=(axis+2)%3;
            for sign in [-1,1] {for layer in 0..self.size[axis] {
                let mut mask=vec![0;self.size[u]*self.size[w]];
                for j in 0..self.size[w]{for i in 0..self.size[u]{
                    let mut q=[0;3];q[axis]=layer;q[u]=i;q[w]=j;
                    let id=self.get(q);if id==0{continue;}
                    let outside=if sign<0{layer==0}else{layer+1==self.size[axis]};
                    let mut neighbor=q;if !outside{neighbor[axis]=(layer as i32+sign) as usize;}
                    if !outside&&self.get(neighbor)!=0{continue;}
                    mask[j*self.size[u]+i]=id;
                    q[axis]=layer+usize::from(sign>0);
                    let mut b=q;b[u]+=1;let mut c=b;c[w]+=1;let mut d=q;d[w]+=1;
                    for (a,b) in [(q,b),(b,c),(c,d),(d,q)]{
                        let key=if a<b{(a,b)}else{(b,a)};edges.entry(key).or_default().push((axis,sign,id));
                    }
                }}
                // Rectángulos del mismo material: una cara, una familia de tramas.
                for j in 0..self.size[w]{for i in 0..self.size[u]{
                    let id=mask[j*self.size[u]+i];if id==0{continue;}
                    let mut width=1;while i+width<self.size[u]&&mask[j*self.size[u]+i+width]==id{width+=1;}
                    let mut height=1;
                    while j+height<self.size[w]&&(i..i+width).all(|k|mask[(j+height)*self.size[u]+k]==id){height+=1;}
                    for y in j..j+height{for x in i..i+width{mask[y*self.size[u]+x]=0;}}
                    let mut a=[0;3];a[axis]=layer+usize::from(sign>0);a[u]=i;a[w]=j;
                    let mut b=a;b[u]+=width;let mut c=b;c[w]+=height;let mut d=a;d[w]+=height;
                    let mut face=vec![at(a),at(b),at(c),at(d)];if sign<0{face.reverse();}
                    // La sombra de una unión es la unión de las sombras de su piel.
                    scene.cast_shadow(&face);
                    let mut n=V3::default();match axis{0=>n.x=sign as f64,1=>n.y=sign as f64,_=>n.z=sign as f64}
                    if scene.camera().facing(n){
                        scene.occluder(&face);
                        let ink=material(id);part(scene,ink);
                        let surface=if axis==2&&sign>0{Surface::Roof}else{Surface::Wall};
                        style.shade(scene,&[face],surface,if id==2{"acento"}else{"trama"});count+=1;
                    }
                }}
            }}
        }
        // Las unidades contiguas se convierten en polilíneas largas antes del render.
        let mut runs=Runs::new();
        for ((a,b),faces) in edges {
            let flat=faces.len()==2&&faces[0]==faces[1];if flat{continue;}
            let visible=faces.iter().any(|(axis,sign,_)|{
                let n=match axis{0=>v(*sign as f64,0.0,0.0),1=>v(0.0,*sign as f64,0.0),_=>v(0.0,0.0,*sign as f64)};
                scene.camera().facing(n)
            });if !visible{continue;}
            let axis=(0..3).find(|i|a[*i]!=b[*i]).unwrap();
            let id=faces.iter().map(|f|f.2).max().unwrap();
            runs.entry((axis,a[(axis+1)%3],a[(axis+2)%3],id)).or_default().push((a[axis],b[axis]));
        }
        for ((axis,u,w,id),mut spans) in runs {
            spans.sort_unstable();let mut merged:Vec<(usize,usize)>=Vec::new();
            for (a,b) in spans{if let Some(last)=merged.last_mut(){if a<=last.1{last.1=last.1.max(b);continue;}}merged.push((a,b));}
            part(scene,material(id));
            for (a,b) in merged{let mut p=[0;3];p[axis]=a;p[(axis+1)%3]=u;p[(axis+2)%3]=w;let mut q=p;q[axis]=b;scene.line(&[at(p),at(q)]);}
        }
        count
    }
}
fn material(id:u8)->&'static str{match id{2=>"acento",3=>"tejados",4=>"terreno",_=>"contorno"}}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::{p,BBox,scene::Camera,generate::{Detail,GenContext}};
    #[test]
    fn united_blocks_have_no_internal_grid_edges_and_carving_reveals_interior(){
        let ctx=GenContext{area:BBox{min:p(0.0,0.0),max:p(100.0,100.0)},seed:0,pen_mm:0.35,detail:Detail::Final};
        let camera=Camera{yaw_deg:225.0,scale:10.0,paper_center:p(50.0,70.0),..Default::default()};
        let mut mass=Mass::new([4,4,6]);mass.fill([0,0,0],[4,4,6],1);
        let render=|mass:&Mass|{let mut scene=Scene::new(camera,ctx.area);mass.draw(&mut scene,Style::new(&ctx,1.0),V3::default(),1.0);scene.render()};
        let whole=render(&mass);assert_eq!(whole.parts[0].lines.len(),9);
        mass.fill([0,0,2],[2,2,4],0);let carved=render(&mass);
        assert!(carved.parts[0].lines.len()>whole.parts[0].lines.len());assert_eq!(carved,render(&mass));
    }
}
