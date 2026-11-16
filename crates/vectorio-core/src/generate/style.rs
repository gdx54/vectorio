//! Familia de escenas Vectorio: cal blanca, hiladas, meridianos y terreno estratificado.
use super::{Detail, GenContext, GenOutput, Params, ParamSpec};
use crate::scene::{Scene, V3, Tone};
use super::kit::OpeningKind;
pub const PARTS: [&str;7]=["contorno","detalle","trama","tejados","sombra","vegetación","terreno"];
/// Grabado: la trama sigue la luz en todas las caras. Cartel: cada tinta es un
/// material (tejas, sombra arrojada, vegetación); conserva blanca la cara al sol.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Look { Engraving, Poster }
#[derive(Clone,Copy,Debug)]
pub struct Style { pub spacing_mm:f64, pub detail:Detail, pub openings:OpeningKind, pub look:Look, pub richness:f64, pub pen_mm:f64 }
#[derive(Clone,Copy,Debug)]
pub enum Surface { Wall, Roof, Ground }
/// Normal por el giro de la cara; tolera vértices alineados al principio.
pub fn normal(face:&[V3])->V3 {
    for i in 1..face.len().saturating_sub(1) {
        let n=(face[i]-face[0]).cross(face[i+1]-face[0]);
        if n.length()>1e-12 {return n.normalized();}
    }
    V3::default()
}
pub fn part(scene:&mut Scene,name:&str){let id=scene.part(name);scene.set_part(id);}
impl Style {
    pub fn new(ctx:&GenContext, spacing_mm:f64)->Self {
        Self{spacing_mm:spacing_mm.max(ctx.pen_mm*1.8),detail:ctx.detail,openings:OpeningKind::Clear,look:Look::Engraving,richness:0.0,pen_mm:ctx.pen_mm}
    }
    /// El pigmento cambia la densidad y los materiales, sin cambiar la geometría.
    pub fn expressive(mut self,params:&Params)->Self {
        self.richness=params.f64("richness").clamp(0.0,1.5);
        if params.choice("look")=="poster" {self.look=Look::Poster;}
        self
    }
    fn safe_gap(self,gap:f64)->f64{gap.max(self.pen_mm*1.45)}
    pub fn prepare(self,scene:&mut Scene){for name in PARTS{scene.part(name);}part(scene,"contorno");}
    /// Alturas y retranqueos usan una unidad común, visible como mínimo a 1,5 mm.
    pub fn quantum(self,scene:&Scene)->f64 {(2.5/scene.camera().scale).max(0.5)}
    pub fn snap(self,scene:&Scene,value:f64)->f64 {let q=self.quantum(scene);(value/q).round()*q}
    /// Separación de la trama de pared por tono: la cara al sol queda en blanco, el
    /// costado en penumbra lleva una trama abierta y la sombra propia, la de serie.
    fn gap(self,tone:Tone)->Option<f64>{
        match tone{
            Tone::Lit=>None,
            Tone::Mid=>Some(self.spacing_mm*2.2),
            Tone::Dark=>Some(self.spacing_mm),
        }
    }
    /// Ángulo de papel de la trama de sombras arrojadas.
    pub fn shadow_angle(self)->f64{if self.look==Look::Poster{0.0}else{-28.0}}
    /// Separación de la trama de sombras arrojadas.
    pub fn shadow_gap(self)->f64{self.safe_gap(self.spacing_mm*if self.look==Look::Poster{0.85}else{1.35}/(1.0+self.richness*0.55))}
    pub fn shade(self,scene:&mut Scene,faces:&[Vec<V3>],surface:Surface,ink:&str){
        let groups:Vec<Vec<Vec<V3>>>=faces.iter().map(|face|vec![face.clone()]).collect();
        self.shade_groups(scene,&groups,surface,ink);
    }
    /// Como `shade`, con cada cara lógica en sus trozos coplanarios. Las caras del
    /// mismo plano se traman juntas: cada línea cruza las juntas de un solo trazo.
    pub fn shade_groups(self,scene:&mut Scene,groups:&[Vec<Vec<V3>>],surface:Surface,ink:&str){
        let poster=self.look==Look::Poster;
        let roof=matches!(surface,Surface::Roof);
        if poster&&!roof&&self.richness==0.0{return;}
        for group in coplanar(groups){
            let n=normal(&group[0]);
            if !scene.camera().facing(n){continue;}
            let tone=scene.tone(n);
            // El lado al sol queda blanco; el costado frío da volumen al cartel.
            if !roof&&tone==Tone::Lit{continue;}
            let color=if roof&&(poster||self.richness>0.0){"tejados"}
                else if poster&&!matches!(surface,Surface::Ground){"sombra"}else{ink};
            part(scene,color);
            let gap=if roof&&(poster||self.richness>0.0){
                if n.z<1e-6{continue;}
                self.spacing_mm*match tone{Tone::Lit=>1.15,Tone::Mid=>0.8,Tone::Dark=>0.62}/(1.0+self.richness*0.5)
            }else{
                let Some(gap)=self.gap(tone) else{continue};
                gap/(1.0+self.richness*0.35)
            };
            let gap=self.safe_gap(gap);
            if n.z.abs()>0.999||matches!(surface,Surface::Ground){
                scene.hatch_surface(&group,if roof{32.0}else{35.0},gap);
            }else{
                let direction=if roof{V3::new(0.0,0.0,-1.0)+n*n.z}else{V3::new(-n.y,n.x,0.0)};
                scene.hatch_faces(&group,direction,gap);
                // Las tejas oscuras llevan juntas más abiertas: textura, sin mancha sólida.
                if roof&&self.richness>0.6&&tone!=Tone::Lit{
                    scene.hatch_faces(&group,V3::new(-n.y,n.x,0.0),self.safe_gap(gap*3.5));
                }
            }
        }
    }
    /// Cada meridiano conserva el tono de la faceta a media altura. Así la frontera
    /// no zigzaguea al cambiar la pendiente del perfil de una cúpula o una copa.
    pub fn lathe(self,scene:&mut Scene,center:V3,profile:&[(f64,f64)],ink:&str)->Vec<Vec<V3>>{
        part(scene,ink);let Some(lathe)=scene.lathe_parts(center,profile,true) else{return Vec::new()};
        let mid=center.z+(profile[0].1+profile[profile.len()-1].1)*0.5;
        let height=|face:&Vec<V3>|face.iter().map(|v|v.z).sum::<f64>()/face.len() as f64;
        let mut groups=[Vec::new(),Vec::new()];
        // Cada meridiano toma el tono de su faceta más cercana a media altura.
        for meridian in 0..lathe.segments{
            let column=lathe.bands.iter().map(|band|&band[meridian]);
            let Some(reference)=column.clone().min_by(|a,b|(height(a)-mid).abs().total_cmp(&(height(b)-mid).abs())) else{continue};
            let index=match scene.tone(normal(reference)){Tone::Lit=>continue,Tone::Mid=>0,Tone::Dark=>1};
            groups[index].extend(column.cloned());
        }
        let roof=(self.look==Look::Poster||self.richness>0.0)&&ink!="vegetación";
        let hatch=if ink=="vegetación"{"vegetación"}else if roof{"tejados"}else{"trama"};part(scene,hatch);
        for (i,faces) in groups.iter().enumerate(){scene.hatch_surface(faces,0.0,self.safe_gap(self.spacing_mm*match (roof,i){(true,0)=>1.0,(true,_)=>0.7,(false,0)=>1.7,_=>1.0}/(1.0+self.richness*0.35)));}
        if !lathe.caps.is_empty(){self.shade(scene,&lathe.caps,Surface::Ground,hatch);}
        lathe.into_faces()
    }
    pub fn finish(self,scene:&Scene)->GenOutput{
        let mut output:GenOutput=scene.render().into();
        for part in &mut output.parts{part.pen_order=Some(match part.name.as_str(){"vegetación"=>1,"sombra"=>usize::MAX,_=>0});}
        output
    }
}
/// Junta los grupos que están en el mismo plano y miran al mismo lado.
fn coplanar(groups:&[Vec<Vec<V3>>])->Vec<Vec<Vec<V3>>>{
    let mut planes:Vec<(V3,V3,Vec<Vec<V3>>)>=Vec::new();
    for group in groups.iter().filter(|g|!g.is_empty()&&!g[0].is_empty()){
        let (n,origin)=(normal(&group[0]),group[0][0]);
        let same=planes.iter_mut().find(|(m,o,_)|m.dot(n)>1.0-1e-9&&(origin-*o).dot(*m).abs()<1e-7);
        match same{
            Some((_,_,pieces))=>pieces.extend(group.iter().cloned()),
            None=>planes.push((n,origin,group.clone())),
        }
    }
    planes.into_iter().map(|(_,_,pieces)|pieces).collect()
}

/// Control compartido; la calidad geométrica nunca se sortea con el pigmento.
pub fn richness_param()->ParamSpec {
    ParamSpec::Range{id:"richness",label:"Intensidad de tinta",min:0.0,max:1.5,step:0.1,default:1.0,random:None,show:None,
        hint:Some("Más color en tejados y sombras, con separación adaptada al rotulador")}
}
pub fn look_param()->ParamSpec {
    ParamSpec::Choice{id:"look",label:"Estilo",options:&[("poster","Cartel"),("engraving","Grabado")],default:"poster",random:false,show:None,hint:None}
}
