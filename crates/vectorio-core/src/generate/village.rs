//! Pueblo encalado en una ladera de bancales. Cada bancal tiene su calle y una
//! hilera de casas adosadas que dan a ella; un eje de escaleras sube por el centro
//! de una calle a la siguiente hasta la plaza de la iglesia. Algún solar rompe la
//! hilera con un huerto o un patio y, a sangre, la ladera sigue más allá de la hoja
//! con bancales de cultivo y olivares.
use super::*;
use super::{kit::{self,v,Church,Cut,Plant,Pose,Roof,Strata,TowerTop},style::{Look,Style,part}};
use crate::{p,scene::{Camera,Scene,V3}};
pub struct Village;
/// Ancho de la calle de cada bancal, entre el muro de abajo y las fachadas.
const STREET:f64=2.4;
/// Medio ancho de la calleja del eje, por donde sube la escalera central.
const LANE:f64=1.4;
/// Medio ancho de los callejones que suben con escalera estrecha entre manzanas.
const ALLEY:f64=0.9;
/// Fondo máximo de una casa.
const DEPTH:f64=7.0;
/// Fondo de la plaza delante de la iglesia, desde la calle.
const PLAZA:f64=7.5;
/// Altura de una planta.
const STOREY:f64=2.8;
/// Anchos de fachada, en metros: casas más anchas que altas.
const WIDTHS:[f64;9]=[4.5,5.0,5.5,6.0,6.5,7.0,7.5,8.0,9.0];
fn roof(prm:&Params,rng:&mut Rng)->Roof{
    let kinds=[Roof::Flat,Roof::Shed,Roof::Gable,Roof::Hip];
    let weights=[prm.f64("flat"),prm.f64("shed"),prm.f64("gable"),prm.f64("hip")];
    let total:f64=weights.iter().sum();
    let mut t=rng.f()*total;
    for (kind,weight) in kinds.iter().zip(weights){
        if t<weight{return *kind;}
        t-=weight;
    }
    Roof::Flat
}
/// Sol por defecto relativo a la vista: da de lleno en la fachada de la calle que
/// se ve (cal blanca) y roza el otro costado visible (trama abierta). `offset`
/// gira ese sol en grados.
fn sun(yaw_deg:f64,offset_deg:f64,height_deg:f64)->V3{
    let (sy,cy)=yaw_deg.to_radians().sin_cos();
    let facade=if sy<0.0{-1.0}else{1.0};
    let side=if cy<0.0{-1.0}else{1.0};
    // Desde la normal de la fachada, 25° hacia el otro costado visible.
    let turn=-facade*side;
    let azimuth=(90.0*facade+25.0*turn+offset_deg).to_radians();
    let height=height_deg.to_radians();
    v(azimuth.cos()*height.cos(),azimuth.sin()*height.cos(),height.sin())
}
#[derive(Clone,Copy,PartialEq)]
enum Row{Fields,Town(usize),Grove}
/// Franja de bancal: tipo, fondo, frente y cota.
#[derive(Clone,Copy)]
struct Band{row:Row,front:f64,back:f64,z:f64}
/// Parcela en planta, con la fachada en `y0`, y cota de su suelo.
#[derive(Clone,Copy)]
struct Lot{x0:f64,x1:f64,y0:f64,y1:f64,z:f64}
impl Lot{
    fn width(&self)->f64{self.x1-self.x0}
    fn depth(&self)->f64{self.y1-self.y0}
    fn center(&self)->f64{(self.x0+self.x1)*0.5}
    fn min(&self)->V3{v(self.x0,self.y0,self.z)}
    /// La misma parcela con otro fondo o retranqueada desde la calle.
    fn span(&self,y0:f64,y1:f64)->Lot{Lot{y0,y1,..*self}}
}
/// Solo se planta lo que puede verse: fuera de la hoja no se gasta tiempo.
fn on_paper(scene:&Scene,point:V3,margin_mm:f64)->bool{
    let (q,_)=scene.camera().project(point);
    let a=scene.paper();
    q.x>a.min.x-margin_mm&&q.x<a.max.x+margin_mm&&q.y>a.min.y-margin_mm&&q.y<a.max.y+margin_mm
}
/// Fachadas de 4 a 9 m que llenan exactamente el tramo [a, b]. A escala diminuta
/// todas se ensanchan para no bajar de `min`.
fn lots(a:f64,b:f64,min:f64,rng:&mut Rng)->Vec<(f64,f64)>{
    let k=(min/4.0).max(1.0);
    let narrowest=4.0*k;
    let mut out=Vec::new();
    let mut x=a;
    while b-x>=narrowest{
        let mut w=kit::pick(rng,&WIDTHS)*k;
        if b-x-w<narrowest{w=b-x;}
        if w>9.5*k{w=(b-x)*0.5;}
        out.push((x,x+w));
        x+=w;
    }
    out
}
/// Tramos edificables de [a, b] tras quitar calles, callejones y plaza.
fn spans(a:f64,b:f64,cuts:&[(f64,f64)])->Vec<(f64,f64)>{
    let mut cuts=cuts.to_vec();
    cuts.sort_by(|p,q|p.0.total_cmp(&q.0));
    let mut out=Vec::new();
    let mut x=a;
    for (c0,c1) in cuts{
        if c0>x{out.push((x,c0.min(b)));}
        x=x.max(c1);
    }
    if b>x{out.push((x,b));}
    out.retain(|(p,q)|q-p>=2.6);
    out
}
/// Cada figura guarda su sitio: nadie se solapa con otra ni con una farola o la fuente.
/// Son las figuras de la ciudad, con su postura, y dejan sombra como los cuerpos.
fn place_person(scene:&mut Scene,taken:&mut Vec<(V3,f64)>,rng:&mut Rng,p:V3,height:f64,poses:&[(Pose,f64)]){
    // El sorteo va antes del hueco, para que un sitio ocupado no cambie a los demás.
    let pose=pick_pose(rng.f(),poses);
    let mirror=rng.f()<0.5;
    let child=rng.f()<0.08;
    if taken.iter().any(|(q,r)|(p-*q).length()<*r){return;}
    let height=if child{height*0.62}else{height};
    if kit::figure_with_shadow(scene,p,height,pose,mirror){
        taken.push((p,0.75f64.max(0.4+1.8/scene.camera().scale)));
    }
}
/// Postura según un reparto acumulado; la última cubre el resto.
fn pick_pose(r:f64,poses:&[(Pose,f64)])->Pose{
    let mut sum=0.0;
    for (pose,weight) in poses{
        sum+=weight;
        if r<sum{return *pose;}
    }
    poses.last().map_or(Pose::Stand,|(pose,_)|*pose)
}
/// En la plaza se para, se pasea y se saluda; en la calle se camina.
const PLAZA_POSES:&[(Pose,f64)]=&[(Pose::Stand,0.45),(Pose::Walk,0.35),(Pose::Wave,0.2)];
const STREET_POSES:&[(Pose,f64)]=&[(Pose::Walk,0.6),(Pose::Stand,0.3),(Pose::Wave,0.1)];
/// Redondea hacia abajo al cuanto: un fondo nunca crece al ajustarlo.
fn snap_down(style:Style,scene:&Scene,value:f64)->f64{
    let q=style.quantum(scene);
    (value/q).floor()*q
}
/// Plano del pueblo, decidido antes que la cámara.
struct Layout{
    width:f64,
    depths:Vec<f64>,
    /// Medio ancho de la plaza.
    half:f64,
    alleys:Vec<f64>,
    church:Option<Church>,
}
impl Layout{
    fn new(prm:&Params,seed:u64)->Self{
        let mut plan=Rng::new(seed^0x5EED);
        let width=prm.f64("size");
        let levels=prm.int("terraces").max(1);
        let half=(width*0.24).clamp(6.0,9.0);
        let church=prm.flag("church").then(||{
            let w=(half*2.0-7.0).clamp(4.0,5.5);
            let top=if prm.choice("tower_top")=="spire"{TowerTop::Spire}else{TowerTop::Dome};
            Church{width:w,length:w*(2.2+plan.f()*0.6),height:6.0,tower:Some((plan.f()<0.5,top))}
        });
        // Calle, hilera de casas y un corral entre la casa y el muro de arriba.
        let mut depths:Vec<f64>=(0..levels).map(|_|STREET+DEPTH+0.8+plan.f()*0.8).collect();
        let top=PLAZA+church.map_or(8.0,|c|c.length)+1.5;
        if let Some(last)=depths.last_mut(){*last=top.max(STREET+DEPTH+4.0);}
        // Dos callejones con escalera estrecha parten las hileras en manzanas.
        let alleys=if width>=26.0{
            let s=(width*(0.27+plan.f()*0.06)).max(half+1.2);
            vec![width*0.5-s,width*0.5+s]
        }else{Vec::new()};
        Self{width,depths,half,alleys,church}
    }
    fn center(&self)->f64{self.width*0.5}
    fn levels(&self)->usize{self.depths.len()}
    fn depth(&self)->f64{self.depths.iter().sum()}
}
struct Ctx<'a>{prm:&'a Params,style:Style}
impl Ctx<'_>{
    /// Altura de una casa: plantas de 2,8 m y, a veces, un cuanto más. Nunca igual
    /// que la vecina: la hilera escalona al menos un cuanto.
    fn house_height(&self,scene:&Scene,previous:Option<f64>,rng:&mut Rng)->f64{
        let style=self.style;
        let q=style.quantum(scene);
        let most=self.prm.int("floors").max(1);
        // Casi la mitad de una planta; cada planta más, algo más rara.
        let mut floors=1;
        while floors<most&&rng.f()<0.55{floors+=1;}
        let mut height=style.snap(scene,floors as f64*STOREY).max(q*2.0);
        if rng.f()<0.3{height+=q;}
        if previous.is_some_and(|h|(h-height).abs()<q*0.5){height+=q;}
        height
    }
    /// Fondo de una casa entre 5 y 7 m, ajustado al cuanto y sin pasar de `room`.
    fn house_depth(&self,scene:&Scene,room:f64,rng:&mut Rng)->f64{
        let depth=snap_down(self.style,scene,5.0+rng.f()*2.0);
        depth.min(snap_down(self.style,scene,room))
    }
    /// Hilera de una calle: casas adosadas a la calle y, de vez en cuando, un solar.
    /// `fade` reduce la densidad hacia los bordes de la ladera.
    fn row(&self,scene:&mut Scene,a:f64,b:f64,street:Lot,fade:&dyn Fn(f64)->f64,rng:&mut Rng){
        let density=self.prm.f64("density");
        let mut previous=None;
        for (x0,x1) in lots(a,b,1.7/scene.camera().scale,rng){
            let lot=Lot{x0,x1,..street};
            let c=lot.center();
            let seen=on_paper(scene,v(c,lot.y0,lot.z),30.0)||on_paper(scene,v(c,lot.y1,lot.z+8.0),30.0);
            if !seen{
                previous=None;
                continue;
            }
            let r=rng.f();
            let d=density*fade(c);
            previous=if r<d{
                Some(self.house(scene,lot,previous,rng))
            }else if fade(c)<0.999&&r>d+(1.0-d)*0.5{
                self.fields(scene,lot.x0,lot.x1,lot.y0,lot.y1,lot.z,None,rng);
                None
            }else if rng.f()<0.4&&lot.depth()>=DEPTH{
                Some(self.patio_house(scene,lot,previous,rng))
            }else{
                self.orchard(scene,lot,rng);
                None
            };
        }
    }
    /// Casa a la calle: volumen con su tejado, zócalo, balcón, macetas y, en la
    /// azotea, algo de vida. Devuelve su altura.
    fn house(&self,scene:&mut Scene,lot:Lot,previous:Option<f64>,rng:&mut Rng)->f64{
        let style=self.style;
        let height=self.house_height(scene,previous,rng);
        let depth=self.house_depth(scene,lot.depth(),rng);
        let kind=roof(self.prm,rng);
        let top=lot.z+height;
        let body=lot.span(lot.y0,lot.y0+depth);
        kit::house(scene,style,body.min(),v(lot.x1,body.y1,top),kind,rng);
        if rng.f()<0.5{kit::plinth(scene,body.min(),v(lot.x1,body.y1,top),0.6);}
        self.roof_life(scene,kind,body,top,rng);
        let floors=(height/STOREY).round().max(1.0);
        if floors>=2.0&&rng.f()<0.4&&lot.width()>=4.0{
            let w=(lot.width()*0.45).min(3.2);
            kit::balcony(scene,style,v(lot.center()-w*0.5,lot.y0,lot.z+height/floors),w,0.55);
        }
        if rng.f()<0.5{kit::pot(scene,style,v(lot.center()+0.75,lot.y0-0.35,lot.z),0.5,rng);}
        height
    }
    /// Casa retranqueada tras un patio con tapia a la calle. Devuelve su altura.
    fn patio_house(&self,scene:&mut Scene,lot:Lot,previous:Option<f64>,rng:&mut Rng)->f64{
        let style=self.style;
        let recess=style.snap(scene,2.5+rng.f()*0.5).max(style.quantum(scene)*2.0);
        let house=lot.span(lot.y0+recess,lot.y1);
        let height=self.house(scene,house,previous,rng);
        let c=lot.center();
        let gate=(lot.width()>=3.0).then_some((c-0.5,c+0.5));
        kit::low_wall(scene,style,lot.min(),v(lot.x1,house.y0,lot.z),1.1,gate,[true,false,true]);
        if rng.f()<0.6{kit::pot(scene,style,v(lot.x0+0.9,lot.y0+recess*0.5,lot.z),0.5,rng);}
        height
    }
    /// Huerto tapiado: surcos delante y un grupo de dos a cuatro olivos al fondo.
    fn orchard(&self,scene:&mut Scene,lot:Lot,rng:&mut Rng){
        let style=self.style;
        let depth=self.house_depth(scene,lot.depth(),rng);
        let c=lot.center();
        let garden=lot.span(lot.y0,lot.y0+depth);
        kit::low_wall(scene,style,garden.min(),v(lot.x1,garden.y1,lot.z),0.9,Some((c-0.5,c+0.5)),[true;3]);
        // Los olivos, juntos, guardan entre copas 1,6 mm en la hoja.
        let pitch=2.4f64.max(1.8/scene.camera().scale+style.quantum(scene)*1.6);
        let columns=((lot.width()-1.2)/pitch).floor().clamp(0.0,2.0) as usize;
        let rows=if columns>0&&rng.f()<0.5&&depth-1.0>=pitch*2.0{2}else{1};
        let grove=garden.y1-0.6-pitch*rows as f64;
        if grove-garden.y0>1.4{
            kit::furrows(scene,v(lot.x0+0.7,garden.y0+0.8,lot.z),v(lot.x1-0.7,grove,lot.z),false,0.45);
        }
        let columns=columns.max(1);
        for i in 0..columns{
            for j in 0..rows{
                let x=c+(i as f64-(columns-1) as f64*0.5)*pitch;
                let y=garden.y1-0.6-pitch*(j as f64+0.5);
                kit::vegetation(scene,style,v(x,y,lot.z),2.5+rng.f()*0.5,Plant::Olive,rng);
            }
        }
    }
    /// En las azoteas, pérgola, tendedero, casetón o macetas.
    fn roof_life(&self,scene:&mut Scene,kind:Roof,lot:Lot,top:f64,rng:&mut Rng){
        if kind!=Roof::Flat{return;}
        let style=self.style;
        // Lo que se pone en la azotea queda a 1,6 mm en papel del pretil.
        let q=style.quantum(scene);
        let apart=1.6/scene.camera().scale;
        let inset=q.max(apart)+apart*1.1+0.2;
        let (a,b)=(v(lot.x0+inset,lot.y0+inset,top),v(lot.x1-inset,lot.y1-inset,top));
        if b.x-a.x<1.2f64.max(apart*2.2)||b.y-a.y<1.0f64.max(apart*2.2){return;}
        let r=rng.f();
        if r<0.3{
            kit::pergola(scene,a,v(b.x,a.y+(b.y-a.y)*0.65,top),2.1);
        }else if r<0.55{
            let y=(a.y+b.y)*0.5;
            kit::laundry(scene,v(a.x+0.2,y,top),v(b.x-0.2,y,top),1.8,rng);
        }else if r<0.85&&b.x-a.x>=2.4&&b.y-a.y>=2.0{
            self.stair_head(scene,a,b,rng);
        }else{
            kit::pot(scene,style,v(b.x-0.4,a.y+0.4,top),0.5,rng);
        }
    }
    /// Casetón de escalera en la esquina trasera de la azotea y, a veces, un depósito.
    fn stair_head(&self,scene:&mut Scene,a:V3,b:V3,rng:&mut Rng){
        let style=self.style;
        let q=style.quantum(scene);
        let (w,d)=(((b.x-a.x)*0.45).min(1.6),((b.y-a.y)*0.5).min(1.3));
        let left=rng.f()<0.5;
        let (x0,x1)=if left{(a.x,a.x+w)}else{(b.x-w,b.x)};
        let height=style.snap(scene,2.1).max(q*2.0);
        kit::stair_head(scene,style,v(x0,b.y-d,a.z),v(x1,b.y,a.z+height));
        if rng.f()>=0.5{return;}
        let x=if left{x1+0.6}else{x0-0.6};
        if x-0.35>a.x&&x+0.35<b.x{kit::water_tank(scene,v(x,b.y-0.5,a.z),0.35,0.8);}
    }
    /// Mosaico de cultivos: surcos en direcciones alternas, olivos en hilera y algún ciprés.
    #[allow(clippy::too_many_arguments)]
    fn fields(&self,scene:&mut Scene,x0:f64,x1:f64,y0:f64,y1:f64,z:f64,avoid:Option<f64>,rng:&mut Rng){
        // Una franja honda se parte en hileras de parcelas para formar un mosaico.
        let mut y=y0;
        while y1-y>14.0{
            let d=8.0+rng.f()*5.0;
            self.field_row(scene,Lot{x0,x1,y0:y,y1:y+d,z},y1,avoid,rng);
            y+=d;
        }
        self.field_row(scene,Lot{x0,x1,y0:y,y1,z},y1,avoid,rng);
    }
    /// Una hilera de parcelas; `edge` es el fondo de la franja entera y `avoid`, el
    /// eje de la escalera, delante del cual no se planta nada.
    fn field_row(&self,scene:&mut Scene,strip:Lot,edge:f64,avoid:Option<f64>,rng:&mut Rng){
        let clear=|p:V3|avoid.is_none_or(|c|(p.x-c).abs()>LANE+1.0||p.y<edge-3.6);
        let avoid=avoid.filter(|_|strip.y1>edge-3.6);
        let mut x=strip.x0;
        while strip.x1-x>1.5{
            let w=(5.0+rng.f()*5.0).min(strip.x1-x);
            let plot=Lot{x0:x+0.5,x1:x+w-0.5,..strip};
            x+=w;
            if !on_paper(scene,v(plot.center(),(plot.y0+plot.y1)*0.5,plot.z),60.0){continue;}
            let r=rng.f();
            if r<0.45{
                self.furrowed(scene,plot,avoid,rng);
            }else if r<0.8{
                self.grove(scene,plot,&clear,rng);
            }else if r<0.9{
                self.pine_stand(scene,plot,&clear,rng);
            }else{
                self.cypress_row(scene,plot.x0,plot.x1,plot.y1-0.9,plot.z,&clear,rng);
            }
        }
    }
    /// Parcela labrada, partida si la cruza el eje de la escalera.
    fn furrowed(&self,scene:&mut Scene,plot:Lot,avoid:Option<f64>,rng:&mut Rng){
        let along=rng.f()<0.5;
        let (y0,y1)=(plot.y0+0.5,plot.y1-0.5);
        let pieces=match avoid{
            Some(c) if plot.x0<c+LANE+1.0&&plot.x1>c-LANE-1.0=>vec![(plot.x0,c-LANE-1.0),(c+LANE+1.0,plot.x1)],
            _=>vec![(plot.x0,plot.x1)],
        };
        for (a,b) in pieces{
            if b-a>1.0{kit::furrows(scene,v(a,y0,plot.z),v(b,y1,plot.z),along,0.55);}
        }
    }
    /// Olivar en retícula: árboles iguales, nunca más juntos que 1,6 mm en papel.
    fn grove(&self,scene:&mut Scene,plot:Lot,clear:&dyn Fn(V3)->bool,rng:&mut Rng){
        // Un tercio de los olivares se deja en blanco y el resto va más abierto: a sangre
        // el campo no debe pesar más que el pueblo.
        if rng.f()<0.33{return;}
        let pitch=4.2f64.max(1.8/scene.camera().scale+self.style.quantum(scene)*1.6);
        let n=(plot.width()/pitch).floor().max(1.0) as usize;
        let m=((plot.depth()-1.0)/pitch).floor().max(1.0) as usize;
        for i in 0..n{
            for j in 0..m{
                let x=plot.x0+(i as f64+0.5)*plot.width()/n as f64;
                let y=plot.y0+0.5+(j as f64+0.5)*(plot.depth()-1.0)/m as f64;
                let p=v(x,y,plot.z);
                if on_paper(scene,p,8.0)&&clear(p){
                    kit::vegetation(scene,self.style,p,2.4+rng.f()*0.5,Plant::Olive,rng);
                }
            }
        }
    }
    /// Pinar ralo: pinos piñoneros sueltos, con las copas a 1,6 mm en papel.
    fn pine_stand(&self,scene:&mut Scene,plot:Lot,clear:&dyn Fn(V3)->bool,rng:&mut Rng){
        let height=5.0+rng.f()*0.8;
        let pitch=height*0.96+1.6/scene.camera().scale;
        let n=(plot.width()/pitch).floor() as usize;
        let m=((plot.depth()-1.0)/pitch).floor().max(1.0) as usize;
        for i in 0..n{
            for j in 0..m{
                // Al tresbolillo y algo movidos, que no parezcan plantados a cordel.
                let shift=if j%2==1{0.25}else{-0.25};
                let x=plot.x0+(i as f64+0.5+shift*0.5)*plot.width()/n as f64+(rng.f()-0.5)*0.6;
                let y=plot.y0+0.5+(j as f64+0.5)*(plot.depth()-1.0)/m as f64+(rng.f()-0.5)*0.6;
                let p=v(x,y,plot.z);
                if on_paper(scene,p,8.0)&&clear(p){
                    kit::vegetation(scene,self.style,p,height+rng.f()*0.4,Plant::Pine,rng);
                }
            }
        }
    }
    /// Hilera de cipreses entre `x0` y `x1`.
    #[allow(clippy::too_many_arguments)]
    fn cypress_row(&self,scene:&mut Scene,x0:f64,x1:f64,y:f64,z:f64,clear:&dyn Fn(V3)->bool,rng:&mut Rng){
        let pitch=1.6f64.max(1.8/scene.camera().scale+self.style.quantum(scene)*0.6);
        let n=((x1-x0)/pitch).floor().max(1.0) as usize;
        for i in 0..n{
            let p=v(x0+(i as f64+0.5)*(x1-x0)/n as f64,y,z);
            if on_paper(scene,p,8.0)&&clear(p){
                kit::vegetation(scene,self.style,p,4.2+rng.f()*0.8,Plant::Cypress,rng);
            }
        }
    }
    /// Plaza de la iglesia: enlosado, fuente en el eje, naranjos con bancos y la
    /// iglesia al fondo, con una pareja de cipreses en el costado libre.
    fn plaza(&self,scene:&mut Scene,square:Lot,church:Option<Church>,taken:&mut Vec<(V3,f64)>,rng:&mut Rng){
        let style=self.style;
        let c=square.center();
        let z=square.z;
        kit::paving(scene,v(square.x0,square.y0+0.2,z),v(square.x1,square.y1-0.2,z),1.6);
        let church_front=square.y0+PLAZA;
        // Bancos, olivos y gente se quedan antes del campanario, que avanza sobre la fachada.
        let front_line=church_front-church.map_or(0.0,|plan|plan.ahead(scene,style));
        let fountain=v(c,square.y0+PLAZA*0.5,z);
        kit::fountain(scene,style,fountain,1.25);
        taken.push((fountain,2.0));
        // La primera pareja, la de la entrada, son palmeras; detrás, olivos.
        let mut y=square.y0+1.6;
        let mut first=true;
        while y<front_line-1.0{
            for x in [square.x0+1.1,square.x1-1.1]{
                if first{
                    kit::vegetation(scene,style,v(x,y,z),5.2+rng.f()*0.4,Plant::Palm,rng);
                }else{
                    kit::vegetation(scene,style,v(x,y,z),3.0+rng.f()*0.4,Plant::Olive,rng);
                }
            }
            first=false;
            if y+1.6<front_line-0.8{
                kit::bench(scene,v(square.x0+2.0,y+0.8,z),false);
                kit::bench(scene,v(square.x1-2.45,y+0.8,z),false);
            }
            y+=3.2;
        }
        if let Some(plan)=church{
            kit::church(scene,style,v(c,church_front,z),plan,rng);
            // La pareja de cipreses va en el costado sin campanario.
            let left=plan.tower.is_some_and(|(left,_)|left);
            let x=c+if left{1.0}else{-1.0}*(plan.width*0.5+1.2);
            for y in [church_front+1.0,church_front+2.8]{
                if x>square.x0+0.6&&x<square.x1-0.6{
                    kit::vegetation(scene,style,v(x,y,z),4.6,Plant::Cypress,rng);
                }
            }
        }
        let people=(self.prm.f64("people")*9.0).round() as usize;
        for _ in 0..people{
            let x=square.x0+2.0+rng.f()*(square.width()-4.0);
            let y=square.y0+0.6+rng.f()*(front_line-square.y0-1.6);
            let height=1.65+rng.f()*0.15;
            place_person(scene,taken,rng,v(x,y,z),height,PLAZA_POSES);
        }
        self.plaza_life(scene,square,front_line,taken,rng);
    }
    /// Un vendedor de globos junto a la fuente y una bandada de palomas encima,
    /// si hay gente en el pueblo.
    fn plaza_life(&self,scene:&mut Scene,square:Lot,front_line:f64,taken:&mut Vec<(V3,f64)>,rng:&mut Rng){
        if self.prm.f64("people")<=0.0{return;}
        let z=square.z;
        let side=if rng.f()<0.5{-1.0}else{1.0};
        let foot=v(square.center()+side*2.6,square.y0+PLAZA*0.5-0.8,z);
        let count=4+(rng.f()*4.0) as usize;
        let free=foot.y<front_line-1.0&&!taken.iter().any(|(q,r)|(foot-*q).length()<*r);
        if free&&kit::balloon_seller(scene,rng,foot,count,1.7){taken.push((foot,1.2));}
        let birds=6+(rng.f()*8.0) as usize;
        let center=v(square.center()+(rng.f()-0.5)*6.0,square.y0+PLAZA*0.5,z+9.0+rng.f()*3.0);
        kit::flock(scene,rng,center,birds);
    }
    /// Calle del bancal: gente repartida y farolas a ambos lados del eje.
    fn street(&self,scene:&mut Scene,street:Lot,center:f64,alleys:&[f64],taken:&mut Vec<(V3,f64)>,rng:&mut Rng){
        let lamps=[center-LANE-0.3,center+LANE+0.3].into_iter().chain(alleys.iter().map(|x|x+ALLEY+0.3));
        for x in lamps{
            let p=v(x,street.y0+0.35,street.z);
            kit::lamp(scene,p,3.4);
            taken.push((p,0.6));
        }
        let count=(self.prm.f64("people")*street.width()/7.0*(0.5+rng.f())).round() as usize;
        for _ in 0..count{
            // Entre el bordillo y las macetas de las puertas.
            let p=v(street.x0+0.5+rng.f()*(street.width()-1.0),street.y0+0.45+rng.f()*0.9,street.z);
            let free=(p.x-center).abs()>LANE&&alleys.iter().all(|x|(p.x-x).abs()>ALLEY);
            let height=1.6+rng.f()*0.2;
            if free{place_person(scene,taken,rng,p,height,STREET_POSES);}
        }
    }
}
/// Caras de cada bancal que dan al exterior de la maqueta, en el orden delantera,
/// derecha, trasera e izquierda, y la cota desde la que se ve su muro delantero.
fn cuts(bands:&[Band])->Vec<Cut>{
    let last=bands.len().saturating_sub(1);
    bands.iter().enumerate().map(|(k,_)|{
        let wall_from=(k>0).then(||bands[k-1].z);
        Cut{exterior:[k==0,true,k==last,true],wall_from}
    }).collect()
}
/// Estilo de la receta: separación de trama, ventanas y aspecto.
fn style(prm:&Params,ctx:&GenContext)->Style{
    let mut style=Style::new(ctx,prm.f64("spacing")).expressive(prm);
    if prm.flag("dark_windows"){style.openings=kit::OpeningKind::Dark;}
    if prm.choice("look")=="poster"{style.look=Look::Poster;}
    style
}
/// Extremos del terreno a sangre: se mide dónde caen las esquinas de la hoja sobre
/// las cotas más baja y más alta y se alarga la ladera hasta pasarlas.
fn bleed(scene:&Scene,ctx:&GenContext,layout:&Layout,rise:f64)->(f64,f64,f64,f64){
    let width=layout.width;
    let depth=layout.depth();
    let (mut x0,mut x1,mut near,mut far)=(-width*0.5,width*1.5,-9.0f64,depth+16.0);
    let (a,cam)=(ctx.area,*scene.camera());
    let pad=2.0f64.max(6.0/cam.scale);
    for corner in [a.min,p(a.max.x,a.min.y),a.max,p(a.min.x,a.max.y)]{
        for z in [-rise,layout.levels() as f64*rise]{
            let g=cam.ground(corner,z);
            x0=x0.min(g.x-pad);
            x1=x1.max(g.x+pad);
            near=near.min(g.y-pad);
            far=far.max(g.y+pad);
        }
    }
    // Un área muy alargada no debe disparar el tamaño del terreno.
    let limit=width*4.0+depth;
    (x0.max(-limit),x1.min(width+limit),near.max(-limit),far.min(depth+limit))
}
/// Escena completa de la receta ya normalizada.
fn scene(prm:&Params,ctx:&GenContext)->Scene{
    let style=style(prm,ctx);
    let layout=Layout::new(prm,ctx.seed);
    let crop=prm.choice("frame")=="crop";
    let flat=prm.choice("relief")=="flat";
    let slope=if flat{0.0}else{prm.f64("slope")};
    let top_z=(layout.levels() as f64-1.0)*slope;
    let yaw=prm.choice("yaw").parse().unwrap_or(225.0);
    let mut camera=Camera{yaw_deg:yaw,elev_deg:prm.choice("elevation").parse().unwrap_or(35.264),..Default::default()};
    // A sangre se encuadra el núcleo, con algo de campo alrededor si se pide, y la
    // ladera desborda la hoja.
    let around=if crop{prm.f64("country")*layout.width*0.6}else{0.0};
    let world=[v(-around,-around,-slope),v(layout.width+around,layout.depth()+around,top_z+14.0)];
    camera.fit(world,ctx.area,if crop{0.0}else{1.0});
    let build=|camera,paper|{
        let mut scene=Scene::new(camera,paper);
        style.prepare(&mut scene);
        scene.set_sun(Some(sun(yaw,prm.f64("sun"),prm.f64("sun_height"))));
        Town{prm,ctx,style,layout:&layout,crop,flat,slope}.draw(&mut scene);
        scene
    };
    if crop{build(camera,ctx.area)}else{fit_scene(ctx.area,camera,build)}
}
/// Lo necesario para dibujar el pueblo con una cámara dada.
struct Town<'a>{prm:&'a Params,ctx:&'a GenContext,style:Style,layout:&'a Layout,crop:bool,flat:bool,slope:f64}
impl Town<'_>{
    fn draw(&self,scene:&mut Scene){
        let style=self.style;
        let rise=if self.flat{0.0}else{style.snap(scene,self.slope).max(style.quantum(scene))};
        let (x0,x1,bands)=self.bands(scene,rise);
        self.terrain(scene,x0,x1,&bands);
        let mut rng=Rng::new(self.ctx.seed);
        let c=Ctx{prm:self.prm,style};
        for (k,band) in bands.iter().enumerate(){
            scene.begin_shadows(band.z,Some([x0,band.front,x1,band.back]),style.shadow_angle());
            let center=self.layout.center();
            match band.row{
                Row::Fields=>c.fields(scene,x0,x1,band.front,band.back,band.z,Some(center),&mut rng),
                Row::Grove=>c.fields(scene,x0,x1,band.front,band.back,band.z,None,&mut rng),
                Row::Town(i)=>self.town_band(scene,&c,*band,i,x0,x1,&mut rng),
            }
            part(scene,"sombra");
            let gap=(style.shadow_gap()/self.prm.f64("shadow")).max(self.ctx.pen_mm*1.3);
            scene.hatch_shadows(gap);
            if k>0&&!self.flat{self.stairs(scene,bands[k-1],*band);}
        }
    }
    /// Franjas de bancal de delante atrás y extremos del terreno en x.
    fn bands(&self,scene:&Scene,rise:f64)->(f64,f64,Vec<Band>){
        let layout=self.layout;
        let (x0,x1,near,far)=if self.crop{
            bleed(scene,self.ctx,layout,rise)
        }else{
            (0.0,layout.width,0.0,layout.depth())
        };
        let mut bands=Vec::new();
        if self.crop{bands.push(Band{row:Row::Fields,front:near,back:0.0,z:-rise});}
        let mut y=0.0;
        for (i,d) in layout.depths.iter().enumerate(){
            bands.push(Band{row:Row::Town(i),front:y,back:y+d,z:i as f64*rise});
            y+=d;
        }
        if self.crop{bands.push(Band{row:Row::Grove,front:y,back:far,z:layout.levels() as f64*rise});}
        (x0,x1,bands)
    }
    /// Bloques macizos de terreno, uno por franja, con estratos comunes en las caras
    /// exteriores. En llano todas comparten cota: un solo bloque.
    fn terrain(&self,scene:&mut Scene,x0:f64,x1:f64,bands:&[Band]){
        let style=self.style;
        let cut=style.snap(scene,self.prm.f64("cut_depth")).max(style.quantum(scene));
        let (first,last)=(bands[0],bands[bands.len()-1]);
        let bottom=first.z-cut;
        let top=bands.iter().map(|b|b.z).fold(bottom,f64::max);
        let mut rng=Rng::new(self.ctx.seed.wrapping_add(1));
        let strata=Strata::new(scene,style,bottom,top,v(x0,first.front,0.0),v(x1,last.back,0.0),&mut rng);
        if self.flat{
            kit::terrace(scene,style,v(x0,first.front,bottom),v(x1,last.back,0.0),Cut::block(),&strata);
            return;
        }
        for (band,cut) in bands.iter().zip(cuts(bands)){
            kit::terrace(scene,style,v(x0,band.front,bottom),v(x1,band.back,band.z),cut,&strata);
        }
    }
    /// Calle, hilera de casas y, en el bancal de arriba, plaza, iglesia y olivares.
    #[allow(clippy::too_many_arguments)]
    fn town_band(&self,scene:&mut Scene,c:&Ctx,band:Band,i:usize,x0:f64,x1:f64,rng:&mut Rng){
        let layout=self.layout;
        let (width,center,half)=(layout.width,layout.center(),layout.half);
        let top=i==layout.levels()-1;
        let mut taken=Vec::new();
        let mut cuts:Vec<(f64,f64)>=layout.alleys.iter().map(|x|(x-ALLEY,x+ALLEY)).collect();
        let square=(center-half,center+half);
        if top{
            let lot=Lot{x0:square.0,x1:square.1,y0:band.front,y1:band.back,z:band.z};
            c.plaza(scene,lot,layout.church,&mut taken,rng);
            cuts.push((square.0-0.3,square.1+0.3));
        }else{
            cuts.push((center-LANE,center+LANE));
        }
        // La hilera se aparta del muro de arriba: tejados y figuras de la calle
        // superior no rozan la trasera.
        let yard=0.8f64.max(1.7/scene.camera().scale);
        let lot_front=band.front+STREET;
        let lot_back=if top{lot_front+DEPTH}else{band.back-yard};
        let street=Lot{x0:0.4,x1:width-0.4,y0:lot_front,y1:lot_back,z:band.z};
        let core=|_:f64|1.0;
        for (a,b) in spans(0.4,width-0.4,&cuts){c.row(scene,a,b,street,&core,rng);}
        if self.crop{
            // A sangre el caserío sigue por los lados y se deshace en huertas.
            let side=width*0.5;
            let fade=|x:f64|{
                let t=if x<0.0{-x}else{x-width};
                (1.0-t/side*0.8).max(0.15)-1e-3
            };
            c.row(scene,x0+0.4,-0.4,street,&fade,rng);
            c.row(scene,width+0.4,x1-0.4,street,&fade,rng);
        }
        if top{self.top_edge(scene,c,band,x0,x1,lot_back+yard,square,rng);}
        let street=Lot{y0:band.front,y1:lot_front,..street};
        c.street(scene,street,center,&layout.alleys,&mut taken,rng);
    }
    /// Detrás de las casas del bancal de arriba, olivares y, en el borde del núcleo,
    /// una hilera de cipreses; nada de ello toca la iglesia.
    #[allow(clippy::too_many_arguments)]
    fn top_edge(&self,scene:&mut Scene,c:&Ctx,band:Band,x0:f64,x1:f64,from:f64,square:(f64,f64),rng:&mut Rng){
        let cypresses=band.back-1.1;
        let fields=cypresses-1.1;
        let everywhere=|_:V3|true;
        let plaza=[(square.0-0.3,square.1+0.3)];
        for (a,b) in spans(x0+0.4,x1-0.4,&plaza){
            if fields-from>=3.0{c.fields(scene,a,b,from,fields,band.z,None,rng);}
        }
        if self.prm.f64("vegetation")<=0.0{return;}
        for (a,b) in spans(0.4,self.layout.width-0.4,&plaza){
            c.cypress_row(scene,a,b,cypresses,band.z,&everywhere,rng);
        }
    }
    /// Escalera del eje y de los callejones: arranca en la calle del bancal de abajo,
    /// sube pegada a su muro y desembarca en la calle de arriba.
    fn stairs(&self,scene:&mut Scene,below:Band,band:Band){
        if !matches!(band.row,Row::Town(_)){return;}
        let style=self.style;
        let center=self.layout.center();
        let rise=band.z-below.z;
        kit::stairs(scene,style,v(center-1.0,below.back-2.4,below.z),2.0,2.4,rise);
        if !matches!(below.row,Row::Town(_)){return;}
        for x in &self.layout.alleys{
            kit::stairs(scene,style,v(x-0.7,below.back-1.8,below.z),1.4,1.8,rise);
        }
    }
}
impl Generator for Village{
    fn id(&self)->&'static str{"white_village"}
    fn name(&self)->&'static str{"Pueblo blanco"}
    fn category(&self)->&'static str{"Escenas"}
    fn description(&self)->&'static str{"Ladera de bancales con calles, plaza, iglesia, huertos y olivares."}
    fn params(&self)->Vec<ParamSpec>{vec![
        ParamSpec::Choice{id:"look",label:"Estilo",options:&[("poster","Cartel"),("engraving","Grabado")],default:"poster",random:false,show:None,
            hint:Some("Cartel: fachada al sol blanca, cubiertas cálidas y sombras frías. Grabado: trama de luz en todas las caras")},
        ParamSpec::Choice{id:"frame",label:"Encuadre",options:&[("crop","A sangre"),("model","Maqueta")],default:"crop",random:false,show:None,
            hint:Some("A sangre: la ladera sigue más allá de la hoja. Maqueta: bloque de terreno cortado")},
        ParamSpec::Range{id:"country",label:"Campo alrededor",min:0.0,max:1.0,step:0.1,default:0.2,random:Some((0.0,0.6)),
            show:Some(Show{id:"frame",equals:"crop"}),hint:Some("Aleja la cámara para que el pueblo quede rodeado de huertas y olivares")},
        ParamSpec::Section{label:"Pueblo y ladera"},
        ParamSpec::Choice{id:"relief",label:"Terreno",options:&[("terraces","Bancales"),("flat","Llano")],default:"terraces",random:false,show:None,
            hint:Some("Bancales: ladera escalonada con muros y escaleras. Llano: el pueblo sobre terreno plano")},
        range("size","Tamaño del pueblo",20.0,44.0,1.0,32.0),
        ParamSpec::Range{id:"terraces",label:"Calles",min:3.0,max:6.0,step:1.0,default:4.0,random:Some((3.0,6.0)),show:None,
            hint:Some("Número de calles; en ladera, cada una en su bancal")},
        ParamSpec::Range{id:"slope",label:"Pendiente",min:0.8,max:2.2,step:0.2,default:1.2,random:Some((0.8,2.2)),
            show:Some(Show{id:"relief",equals:"terraces"}),hint:None},
        range("density","Densidad de casas",0.35,0.95,0.05,0.85),
        range("floors","Plantas máximas",1.0,3.0,1.0,2.0),
        range("vegetation","Vegetación",0.0,1.0,0.1,0.5),
        range("people","Gente en la calle",0.0,1.0,0.1,0.5),
        range("cut_depth","Profundidad del terreno",0.8,3.0,0.2,1.6),
        ParamSpec::Toggle{id:"dark_windows",label:"Ventanas oscuras",default:true,random:Some(0.7),show:None,
            hint:Some("Rellena ventanas y puertas con trama densa; si no, quedan en blanco")},
        ParamSpec::Section{label:"Tejados"},
        range("flat","Azoteas con pretil",0.0,1.0,0.1,0.3),
        range("shed","Tejados a un agua",0.0,1.0,0.1,0.15),
        range("gable","Tejados a dos aguas",0.0,1.0,0.1,0.4),
        range("hip","Tejados a cuatro aguas",0.0,1.0,0.1,0.15),
        ParamSpec::Toggle{id:"church",label:"Iglesia",default:true,random:Some(0.85),show:None,hint:None},
        ParamSpec::Choice{id:"tower_top",label:"Remate del campanario",options:&[("dome","Cúpula"),("spire","Chapitel")],
            default:"dome",random:true,show:Some(Show{id:"church",equals:"true"}),hint:None},
        ParamSpec::Section{label:"Cámara y luz"},
        choice("yaw","Vista",&[("225","Frontal"),("315","Lateral"),("45","Posterior"),("135","Cuarta vista")],"225"),
        choice("elevation","Elevación",&[("35.26438968","Isométrica"),("26.57","Dimétrica")],"35.26438968"),
        ParamSpec::Range{id:"sun",label:"Giro del sol",min:-90.0,max:90.0,step:5.0,default:0.0,random:Some((-30.0,30.0)),show:None,
            hint:Some("Relativo a la luz de serie, que deja blancas las fachadas de la calle en cualquier vista")},
        range("sun_height","Altura del sol",20.0,70.0,5.0,35.0),
        range("spacing","Separación de trama",0.7,3.0,0.1,0.9),
        ParamSpec::Range{id:"shadow",label:"Densidad de sombras",min:0.3,max:3.0,step:0.1,default:1.0,random:Some((0.5,2.0)),show:None,
            hint:Some("Más alto, líneas de sombra más juntas; más bajo, más abiertas")},
        super::style::richness_param(),
        ParamSpec::Version{current:1},
    ]}
    fn has_volume(&self)->bool{true}
    fn scene(&self,params:&Params,ctx:&GenContext)->Result<Scene,String>{
        if !ctx.valid(){return Err("Área o rotulador no válido".into());}
        let prm=params.normalized(&self.params());
        Ok(scene(&prm,ctx))
    }
    fn generate(&self,params:&Params,ctx:&GenContext)->Result<GenOutput,String>{
        if !ctx.valid(){return Err("Área o rotulador no válido".into());}
        let prm=params.normalized(&self.params());
        let scene=scene(&prm,ctx);
        let mut output=style(&prm,ctx).finish(&scene);
        if cfg!(any(test,debug_assertions)){
            let issues=scene.construction_issues(1.5);
            if !issues.is_empty(){output.warning=Some(issues.join("; "));}
        }
        Ok(output)
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::scene::Tone;
    const YAWS:[f64;4]=[225.0,315.0,45.0,135.0];
    fn context(seed:u64)->GenContext{
        GenContext{area:crate::BBox{min:p(15.0,15.0),max:p(195.0,282.0)},seed,pen_mm:0.4,detail:Detail::Final}
    }
    #[test]
    fn default_sun_whitens_the_street_facade_and_grazes_the_other_side_in_every_view(){
        for yaw in YAWS{
            let mut scene=Scene::new(Camera{yaw_deg:yaw,..Default::default()},context(0).area);
            scene.set_sun(Some(sun(yaw,0.0,35.0)));
            let (sy,cy)=yaw.to_radians().sin_cos();
            let facade=v(0.0,sy.signum(),0.0);
            let side=v(cy.signum(),0.0,0.0);
            assert!(scene.camera().facing(facade)&&scene.camera().facing(side),"{yaw}");
            assert_eq!(scene.tone(facade),Tone::Lit,"{yaw}");
            assert_eq!(scene.tone(side),Tone::Mid,"{yaw}");
        }
    }
    #[test]
    fn at_least_forty_percent_of_the_visible_wall_stays_white_in_every_view(){
        let generator=Village;
        for yaw in YAWS{
            for frame in ["crop","model"]{
                let mut params=Params::default();
                params.set("look",ParamValue::Text("engraving".into()));
                params.set("yaw",ParamValue::Text(format!("{yaw}")));
                params.set("frame",ParamValue::Text(frame.into()));
                let prm=params.normalized(&generator.params());
                let scene=scene(&prm,&context(3));
                let white=scene.lit_wall_fraction("contorno",2.0);
                eprintln!("vista {yaw}, {frame}: {:.0} % de cal",white*100.0);
                assert!(white>=0.4,"vista {yaw}, {frame}: {:.0} % de cal",white*100.0);
            }
        }
    }
    #[test]
    fn houses_stay_in_rows_of_four_to_nine_metres(){
        let mut rng=Rng::new(9);
        for (a,b) in [(0.0,30.0),(3.0,11.5),(-20.0,-0.4)]{
            let lots=lots(a,b,1.0,&mut rng);
            assert!((lots[0].0-a).abs()<1e-9&&(lots.last().unwrap().1-b).abs()<1e-9);
            for pair in lots.windows(2){assert_eq!(pair[0].1,pair[1].0);}
            for (x0,x1) in lots{assert!((4.0..=9.5).contains(&(x1-x0)),"{x0}..{x1}");}
        }
    }
}
