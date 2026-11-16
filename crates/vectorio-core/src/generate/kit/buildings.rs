use super::*;
#[derive(Clone,Copy,Debug,PartialEq)]
pub enum Roof{Flat,Shed,Gable,Hip}
#[derive(Clone,Copy,Debug)]
pub enum TowerTop{Dome,Spire}
/// Hueco claro (marco sobre la pared encalada), oscuro (relleno denso) u oscuro
/// con montante central, como el de un campanario.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum OpeningKind{Clear,Dark,Twin}
/// Un hueco es un único trazo cerrado y atómico, aunque tenga montantes; nunca se
/// recorta una ventana. Además aparta la trama y las sombras de la pared: el
/// interior queda limpio o con su propio relleno.
pub fn opening(scene:&mut Scene,style:Style,origin:V3,horizontal:V3,width:f64,height:f64,kind:OpeningKind){
    if !origin.finite()||!horizontal.finite()||!width.is_finite()||!height.is_finite()||width<=0.0||height<=0.0||
        !scene.visible(width.min(height),1.0)||style.detail==Detail::Preview{return;}
    let h=horizontal.normalized()*width;let up=v(0.0,0.0,height);
    let paper=scene.camera().project(origin).0;
    if paper.dist(scene.camera().project(origin+h).0)<1.0 || paper.dist(scene.camera().project(origin+up).0)<1.0{return;}
    part(scene,"detalle");let mut face=[origin,origin+h,origin+h+up,origin+up];
    // El relleno se traza solo en la cara que mira a la cámara; la de una fachada
    // trasera queda oculta por el propio edificio.
    if !scene.camera().facing(h.cross(up)){face.reverse();}
    scene.hole(&face);scene.whole_line(&[face[0],face[1],face[2],face[3],face[0]]);
    if kind!=OpeningKind::Clear{scene.hatch_hole(&face,up,style.spacing_mm*0.6);}
    if kind==OpeningKind::Twin{
        // El montante sigue siendo atómico: va unido al marco.
        let mid=origin+h*0.5;scene.whole_line(&[origin,mid,mid+up,origin+h+up,origin+up,origin]);
    }
}
/// Caja insertada deliberadamente en el faldón; el motor aporta la junta con el tejado.
pub fn chimney(scene:&mut Scene,style:Style,min:V3,max:V3){box_piece(scene,style,min,max,"contorno");}
/// Grosor de un pretil: un cuanto, sin pasar de un quinto de la planta y sin bajar
/// de 1,6 mm en la hoja, o se confundirían sus dos caras. `None` si no cabe.
fn parapet_thickness(scene:&Scene,style:Style,w:f64,d:f64)->Option<f64>{
    let t=style.quantum(scene).min(w.min(d)*0.2).max(super::street::apart(scene));
    (w.min(d)>=t*3.0).then_some(t)
}
/// Pretil suelto: un anillo de muro de un solo sólido, sin juntas en las esquinas.
pub fn parapet(scene:&mut Scene,style:Style,min:V3,max:V3){
    if !dimensions(min,max){return;}
    let Some(t)=parapet_thickness(scene,style,max.x-min.x,max.y-min.y) else{return};
    part(scene,"contorno");
    let groups=scene.polyhedron(&ring_faces(min,max,t));
    shade_solid(scene,style,&groups,"contorno");
}
/// Losa de alero: una caja fina que vuela sobre la fachada. Si su canto no llega
/// a 1,5 mm en la hoja no se dibuja, porque se leería como una raya suelta.
pub fn eave(scene:&mut Scene,style:Style,min:V3,max:V3){
    if !dimensions(min,max)||!scene.visible(max.z-min.z,1.5){return;}
    box_piece(scene,style,min,max,"contorno");
}
/// Vuelo del tejado sobre la fachada de la calle y canto del faldón, en metros.
/// Ambos guardan 1,6 mm en la hoja; si a esta escala serían desproporcionados, el
/// tejado queda a haces de la fachada, como antes.
fn overhang(scene:&Scene,d:f64)->Option<(f64,f64)>{
    let apart=super::street::apart(scene);
    let (o,t)=(0.4f64.max(apart*1.1),0.18f64.max(1.9/scene.camera().scale));
    (o<=0.8f64.min(d*0.25)&&t<=0.6).then_some((o,t))
}
/// Casa con el tejado volado sobre su fachada delantera (`min.y`). En el tejado a
/// dos aguas la cumbrera va paralela a la calle y los hastiales dan a las vecinas.
pub fn house(scene:&mut Scene,style:Style,min:V3,max:V3,roof:Roof,rng:&mut Rng){
    let trim=Trim{chimneys:true,eaves:true,ridge_along_x:true,facade:true};
    building(scene,style,min,max,roof,trim,rng);
}
/// Casa sin vuelo: para las que tienen delante, a poca distancia, otro volumen.
pub fn house_flush(scene:&mut Scene,style:Style,min:V3,max:V3,roof:Roof,rng:&mut Rng){
    let trim=Trim{chimneys:true,eaves:false,ridge_along_x:true,facade:true};
    building(scene,style,min,max,roof,trim,rng);
}
/// Volumen de azotea con pretil y sin huecos: las fachadas las pone quien lo llama.
pub fn house_bare(scene:&mut Scene,style:Style,min:V3,max:V3,rng:&mut Rng){
    let trim=Trim{chimneys:false,eaves:false,ridge_along_x:true,facade:false};
    building(scene,style,min,max,Roof::Flat,trim,rng);
}
/// Nave de iglesia: sin chimenea, con la fachada a haces del tejado y el hastial
/// hacia `min.y`, la plaza. Sus vanos los pone `church`.
pub fn nave(scene:&mut Scene,style:Style,min:V3,max:V3,rng:&mut Rng){
    let trim=Trim{chimneys:false,eaves:false,ridge_along_x:false,facade:false};
    building(scene,style,min,max,Roof::Gable,trim,rng);
}
/// Planta de una iglesia: ancho y largo de la nave, altura de sus muros y, si lleva
/// campanario, si va en la esquina izquierda de los pies y con qué remate.
#[derive(Clone,Copy,Debug)]
pub struct Church{pub width:f64,pub length:f64,pub height:f64,pub tower:Option<(bool,TowerTop)>}
impl Church{
    /// Metros que el campanario avanza por delante de la fachada; 0 sin torre.
    pub fn ahead(&self,scene:&Scene,style:Style)->f64{
        let Some((left,_))=self.tower else{return 0.0};
        let place=tower_place(scene,style,v(0.0,0.0,0.0),*self,left);
        place.map_or(0.0,|t|t.width-t.reach)
    }
}
/// Iglesia con la fachada hacia `min.y`; `front` es el centro de la fachada a ras
/// de suelo. El campanario se encastra en una esquina de los pies: avanza sobre la
/// fachada y asoma por el costado, y el motor dibuja su encuentro con el muro y el
/// faldón de la nave.
pub fn church(scene:&mut Scene,style:Style,front:V3,plan:Church,rng:&mut Rng){
    if !front.finite()||![plan.width,plan.length,plan.height].iter().all(|n|n.is_finite()&&*n>0.0&&*n<1000.0){return;}
    let half=plan.width*0.5;
    let min=v(front.x-half,front.y,front.z);
    let max=v(front.x+half,front.y+plan.length,front.z+plan.height);
    nave(scene,style,min,max,rng);
    let tower=plan.tower.and_then(|(left,top)|tower_place(scene,style,front,plan,left).map(|t|(t,top)));
    church_openings(scene,style,min,max,tower.map(|(t,_)|t));
    let Some((place,top))=tower else{return};
    let height=(plan.height*2.1).max(plan.height+4.0);
    bell_tower(scene,style,place.center,place.width,height,top);
}
/// Sitio del campanario: centro a ras de suelo, ancho y el fondo que ocupa sobre
/// el costado de la nave.
#[derive(Clone,Copy,Debug)]
struct TowerPlace{center:V3,width:f64,reach:f64}
/// El campanario avanza sobre la fachada y entra en la nave lo bastante para que
/// cada cara guarde 1,6 mm en la hoja con la paralela de la nave. `None` si no cabe.
fn tower_place(scene:&Scene,style:Style,front:V3,plan:Church,left:bool)->Option<TowerPlace>{
    let q=style.quantum(scene);
    let apart=super::street::apart(scene)*1.1;
    let width=style.snap(scene,2.1).max(q*2.0);
    let ahead=(width*0.3).max(apart);
    let inside=(width*0.35).max(apart);
    if width>plan.width*0.6||width-inside<apart||width-ahead<apart{return None;}
    let side=if left{-1.0}else{1.0};
    let x=front.x+side*(plan.width*0.5-inside+width*0.5);
    let y=front.y-ahead+width*0.5;
    Some(TowerPlace{center:v(x,y,front.z),width,reach:front.y-ahead+width})
}
/// Portada y óculo en la fachada; ventanas altas y estrechas a lo largo de los
/// costados, salvo donde está el campanario.
fn church_openings(scene:&mut Scene,style:Style,min:V3,max:V3,tower:Option<TowerPlace>){
    let (w,length,height)=(max.x-min.x,max.y-min.y,max.z-min.z);
    let center=(min.x+max.x)*0.5;
    let door=(w*0.28).min(1.6);
    opening(scene,style,v(center-door*0.5,min.y,min.z),v(1.0,0.0,0.0),door,(height*0.48).min(3.2),style.openings);
    let eye=(w*0.16).min(0.9);
    opening(scene,style,v(center-eye*0.5,min.y,min.z+height*0.7),v(1.0,0.0,0.0),eye,eye,style.openings);
    let clear=tower.map_or(min.y,|t|t.reach+super::street::apart(scene));
    let (tall,narrow)=((height*0.3).min(1.8),0.5);
    let bays=((length-1.5)/3.0).floor().max(1.0) as usize;
    for bay in 0..bays{
        let y=min.y+length*(bay as f64+0.5)/bays as f64-narrow*0.5;
        if y<clear{continue;}
        for x in [min.x,max.x]{
            opening(scene,style,v(x,y,min.z+height*0.45),v(0.0,1.0,0.0),narrow,tall,style.openings);
        }
    }
}
/// Remates opcionales de un edificio y si lleva los huecos de una casa.
#[derive(Clone,Copy)]
struct Trim{chimneys:bool,eaves:bool,ridge_along_x:bool,facade:bool}
/// Uno de los valores, al azar.
pub fn pick<T:Copy>(rng:&mut Rng,values:&[T])->T{
    values[((rng.f()*values.len() as f64) as usize).min(values.len()-1)]
}
fn building(scene:&mut Scene,style:Style,min:V3,max:V3,roof:Roof,trim:Trim,rng:&mut Rng){
    if !dimensions(min,max){return;}
    let q=style.quantum(scene);
    let height=style.snap(scene,max.z-min.z).max(q);
    let max=v(max.x,max.y,min.z+height);
    let (w,d)=(max.x-min.x,max.y-min.y);
    let across=trim.ridge_along_x&&roof==Roof::Gable;
    // Pendientes de teja árabe: unos 30° a dos aguas y algo menos a un agua.
    let rise=match roof{
        Roof::Gable if across=>d*0.28,
        Roof::Shed=>d*0.25,
        _=>w.min(d)*0.35,
    };
    let rise=style.snap(scene,rise).max(q*2.0);
    // El tejado vuela solo hacia la calle: los costados tocan a las casas vecinas y
    // la trasera, al muro del bancal. El canto es una banda vertical bajo el faldón.
    let eaves=trim.eaves&&roof!=Roof::Flat;
    let (overhang,fascia)=if eaves{self::overhang(scene,d).unwrap_or((0.0,0.0))}else{(0.0,0.0)};
    let shell=Shell{min,max,rise,overhang,fascia,across};
    let faces=match roof{
        Roof::Flat=>match parapet_thickness(scene,style,w,d){
            Some(thickness)=>shell.flat(q*2.0,thickness),
            None=>shell.flat(0.0,0.0),
        },
        Roof::Gable=>shell.gable(),
        Roof::Shed=>shell.shed(),
        Roof::Hip=>shell.hip(),
    };
    // Muros, hastiales, canto y tejado son un único sólido: entre el muro y el
    // hastial o el pretil no queda ninguna junta.
    part(scene,"contorno");
    let groups=scene.polyhedron(&faces);
    shade_solid(scene,style,&groups,"contorno");
    if trim.facade{facade(scene,style,min,max,rng);}
    if trim.chimneys{chimney_on(scene,style,&shell,roof,rng);}
}
/// Envolvente para recetas con una gramática de fachada propia.
pub fn house_shell(scene:&mut Scene,style:Style,min:V3,max:V3,roof:Roof,rng:&mut Rng){
    let trim=Trim{chimneys:false,eaves:true,ridge_along_x:false,facade:false};
    building(scene,style,min,max,roof,trim,rng);
}
/// Huecos de una casa. El ritmo (paso, ancho y alto) es regular dentro de la casa
/// y cambia de una a otra; la puerta va en la planta baja, hacia la calle. Las
/// fachadas traseras se descartan enteras por ocultación.
fn facade(scene:&mut Scene,style:Style,min:V3,max:V3,rng:&mut Rng){
    let (w,height)=(max.x-min.x,max.z-min.z);
    let floors=(height/2.6).floor().max(1.0) as usize;
    let storey=height/floors as f64;
    let pitch=pick(rng,&[1.5,1.8,2.2]);
    let width=pick(rng,&[0.5,0.6,0.75]);
    let tall=(storey*0.32).min(0.95);
    let slots=((w-0.6)/pitch).floor().max(1.0) as usize;
    let first=min.x+w*0.5-(slots-1) as f64*pitch*0.5;
    let door_slot=((rng.f()*slots as f64) as usize).min(slots-1);
    let has_door=w>=1.2;
    for floor in 0..floors{
        let z=min.z+floor as f64*storey+storey*0.38;
        for slot in 0..slots{
            let x=first+slot as f64*pitch-width*0.5;
            for y in [min.y,max.y]{
                if has_door&&floor==0&&slot==door_slot&&y==min.y{continue;}
                opening(scene,style,v(x,y,z),v(1.0,0.0,0.0),width,tall,style.openings);
            }
        }
    }
    if has_door{
        let x=first+door_slot as f64*pitch-0.4;
        opening(scene,style,v(x,min.y,min.z),v(1.0,0.0,0.0),0.8,(storey*0.75).min(2.1),style.openings);
    }
    // Algún hueco aislado en las medianeras: solo se ve donde la vecina es más baja.
    if floors>=2&&rng.f()<0.3{
        let y=min.y+(max.y-min.y)*(0.3+rng.f()*0.4)-width*0.5;
        let z=min.z+(floors-1) as f64*storey+storey*0.38;
        for x in [min.x,max.x]{
            opening(scene,style,v(x,y,z),v(0.0,1.0,0.0),width,tall,style.openings);
        }
    }
}
/// Pocas chimeneas y solo en tejados inclinados. La base sale de la altura del
/// faldón en el punto más bajo de su huella y queda hundida en él; sus caras guardan
/// 1,6 mm en papel con los hastiales y la trasera.
fn chimney_on(scene:&mut Scene,style:Style,shell:&Shell,roof:Roof,rng:&mut Rng){
    let q=style.quantum(scene);
    let apart=super::street::apart(scene);
    let (a,b)=(shell.min,shell.max);
    let (w,d)=(b.x-a.x,b.y-a.y);
    let fits=w>=q*7.0&&w>2.8&&d>2.8&&d*0.5-q>=apart*1.05;
    if roof==Roof::Flat||!fits||rng.f()>=0.15{return;}
    let band=b.z+shell.fascia;
    let front=a.y-shell.overhang;
    let across=roof==Roof::Gable&&shell.across;
    let center=if across{(front+b.y)*0.5}else{a.y+d*0.5};
    let (y0,y1)=(center-q,center+q);
    // Altura del faldón en el borde más bajo de la huella de la chimenea.
    let lowest=match roof{
        Roof::Gable if across=>band+shell.rise*(1.0-q/((b.y-front)*0.5)),
        Roof::Shed=>band+shell.rise*(y0-front)/(b.y-front),
        _=>band+shell.rise*q/(w*0.5),
    };
    let base=(band+lowest)*0.5;
    chimney(scene,style,v(a.x+q,y0,base),v(a.x+q*3.0,y1,band+shell.rise+q*2.0));
}
pub fn bell_tower(scene:&mut Scene,style:Style,origin:V3,width:f64,height:f64,top:TowerTop){
    if !origin.finite()||!width.is_finite()||!height.is_finite()||width<=0.0||height<=0.0{return;}
    let height=style.snap(scene,height);let q=style.quantum(scene);let width=style.snap(scene,width).max(q*2.0);let half=width*0.5;
    let min=origin-v(half,half,0.0);let max=origin+v(half,half,height);
    box_piece(scene,style,min,max,"contorno");
    let z=origin.z+height*0.72;
    for y in [min.y,max.y]{opening(scene,style,v(origin.x-width*0.18,y,z),v(1.0,0.0,0.0),width*0.36,height*0.15,OpeningKind::Twin);}
    for x in [min.x,max.x]{opening(scene,style,v(x,origin.y-width*0.18,z),v(0.0,1.0,0.0),width*0.36,height*0.15,OpeningKind::Twin);}
    let profile=match top{
        TowerTop::Dome=>vec![(half,0.0),(half*0.95,width*0.25),(half*0.65,width*0.47),(0.0,width*0.6)],
        TowerTop::Spire=>vec![(half,0.0),(half*0.65,q*2.0),(half*0.82,q*4.0),(half*0.4,q*6.0),(0.0,width*1.5+q*6.0)],
    };
    style.lathe(scene,v(origin.x,origin.y,origin.z+height),&profile,"contorno");
}
/// Casetón de escalera sobre una azotea: caja encalada con su puerta.
pub fn stair_head(scene:&mut Scene,style:Style,min:V3,max:V3){
    if !dimensions(min,max){return;}
    box_piece(scene,style,min,max,"contorno");
    let (w,h)=(max.x-min.x,max.z-min.z);let door=(w*0.45).min(0.7);
    opening(scene,style,v((min.x+max.x)*0.5-door*0.5,min.y,min.z),v(1.0,0.0,0.0),door,(h*0.8).min(1.8),style.openings);
}
/// Depósito de agua: cilindro con tapa abombada apoyado en la azotea.
pub fn water_tank(scene:&mut Scene,origin:V3,radius:f64,height:f64){
    if !origin.finite()||![radius,height].iter().all(|n|n.is_finite()&&*n>0.0&&*n<100.0)||!scene.visible(radius*2.0,1.5){return;}
    let lid=radius*0.35f64.max(super::street::apart(scene)/radius*0.5);
    part(scene,"contorno");scene.lathe(origin,&[(radius,0.0),(radius,height),(radius*0.55,height+lid*0.7),(0.0,height+lid)],true);
}
/// Zócalo pintado: una línea alrededor del volumen a la altura indicada. Las puertas
/// la interrumpen porque el hueco aparta los trazos de su pared.
pub fn plinth(scene:&mut Scene,min:V3,max:V3,height:f64){
    if !dimensions(min,max)||!height.is_finite()||height<=0.0||height>=max.z-min.z||!scene.visible(height,1.0){return;}
    let z=min.z+height;part(scene,"detalle");
    scene.closed(&[v(min.x,min.y,z),v(max.x,min.y,z),v(max.x,max.y,z),v(min.x,max.y,z)]);
}
/// Volumen de una casa con su cubierta, como caras lógicas de un solo poliedro.
/// `overhang` es el vuelo hacia `min.y` y `fascia`, el canto bajo el faldón; con
/// vuelo nulo, el tejado queda a haces de las fachadas.
struct Shell{min:V3,max:V3,rise:f64,overhang:f64,fascia:f64,across:bool}
impl Shell{
    fn corners(&self,z:f64)->[V3;4]{
        let (a,b)=(self.min,self.max);
        [v(a.x,a.y,z),v(b.x,a.y,z),v(b.x,b.y,z),v(a.x,b.y,z)]
    }
    /// Muros de una caja hasta la cota de cada esquina, con la base.
    fn walls(&self,heights:[f64;4])->Vec<Vec<V3>>{
        let base=self.corners(self.min.z);
        let mut faces=vec![base.to_vec()];
        for k in 0..4{
            let j=(k+1)%4;
            let up=|q:V3,z:f64|v(q.x,q.y,z);
            faces.push(vec![base[k],base[j],up(base[j],heights[j]),up(base[k],heights[k])]);
        }
        faces
    }
    /// Azotea: muros exteriores de altura completa, corona del pretil, muros
    /// interiores y suelo hundido. Sin pretil, una caja.
    fn flat(&self,parapet:f64,thickness:f64)->Vec<Vec<V3>>{
        let top=self.max.z;
        if parapet<=0.0{
            let mut faces=self.walls([top;4]);
            faces.push(self.corners(top).to_vec());
            return faces;
        }
        let crown=top+parapet;
        let mut faces=self.walls([crown;4]);
        let ring=ring_faces(v(self.min.x,self.min.y,top),v(self.max.x,self.max.y,crown),thickness);
        // Del anillo se quedan los muros interiores (índice 1) y la corona (2).
        faces.extend(ring.into_iter().enumerate().filter(|(i,_)|i%4==1||i%4==2).map(|(_,face)|face));
        let t=thickness;
        faces.push(vec![v(self.min.x+t,self.min.y+t,top),v(self.max.x-t,self.min.y+t,top),
            v(self.max.x-t,self.max.y-t,top),v(self.min.x+t,self.max.y-t,top)]);
        faces
    }
    /// Caras comunes a las cubiertas con vuelo: fachada delantera bajo el alero,
    /// sofito, costados en L (muro y canto) y base.
    fn eaves(&self,back_top:f64)->Vec<Vec<V3>>{
        let (a,b,top)=(self.min,self.max,self.max.z);
        let (front,band)=(a.y-self.overhang,top+self.fascia);
        let mut faces=vec![self.corners(a.z).to_vec()];
        faces.push(vec![v(a.x,a.y,a.z),v(b.x,a.y,a.z),v(b.x,a.y,top),v(a.x,a.y,top)]);
        faces.push(vec![v(a.x,front,top),v(b.x,front,top),v(b.x,a.y,top),v(a.x,a.y,top)]);
        for x in [a.x,b.x]{
            faces.push(vec![v(x,a.y,a.z),v(x,b.y,a.z),v(x,b.y,back_top),v(x,front,band),v(x,front,top),v(x,a.y,top)]);
        }
        faces
    }
    fn gable(&self)->Vec<Vec<V3>>{
        if self.across{return self.gable_across();}
        let (a,b,top)=(self.min,self.max,self.max.z);
        let mid=(a.x+b.x)*0.5;
        if self.overhang<=0.0{
            let profile=[v(a.x,a.y,a.z),v(b.x,a.y,a.z),v(b.x,a.y,top),v(mid,a.y,top+self.rise),v(a.x,a.y,top)];
            return prism_faces(&profile,v(0.0,b.y-a.y,0.0));
        }
        let (front,band)=(a.y-self.overhang,top+self.fascia);let ridge=band+self.rise;
        let mut faces=self.eaves(band);
        faces.push(vec![v(a.x,front,top),v(b.x,front,top),v(b.x,front,band),v(mid,front,ridge),v(a.x,front,band)]);
        faces.push(vec![v(a.x,b.y,a.z),v(b.x,b.y,a.z),v(b.x,b.y,band),v(mid,b.y,ridge),v(a.x,b.y,band)]);
        for x in [a.x,b.x]{faces.push(vec![v(x,front,band),v(mid,front,ridge),v(mid,b.y,ridge),v(x,b.y,band)]);}
        faces
    }
    /// Dos aguas con la cumbrera a lo largo de x: un prisma con el perfil de la
    /// casa. Con vuelo, el faldón delantero baja hasta el canto y el sofito.
    fn gable_across(&self)->Vec<Vec<V3>>{
        let (a,b,top)=(self.min,self.max,self.max.z);
        let at=|y:f64,z:f64|v(a.x,y,z);
        let profile=if self.overhang<=0.0{
            let ridge=at((a.y+b.y)*0.5,top+self.rise);
            vec![at(a.y,a.z),at(b.y,a.z),at(b.y,top),ridge,at(a.y,top)]
        }else{
            let (front,band)=(a.y-self.overhang,top+self.fascia);
            let ridge=at((front+b.y)*0.5,band+self.rise);
            vec![at(a.y,a.z),at(b.y,a.z),at(b.y,band),ridge,at(front,band),at(front,top),at(a.y,top)]
        };
        prism_faces(&profile,v(b.x-a.x,0.0,0.0))
    }
    fn shed(&self)->Vec<Vec<V3>>{
        let (a,b,top)=(self.min,self.max,self.max.z);
        let profile=if self.overhang<=0.0{
            vec![v(a.x,a.y,a.z),v(a.x,b.y,a.z),v(a.x,b.y,top+self.rise),v(a.x,a.y,top)]
        }else{
            let (front,band)=(a.y-self.overhang,top+self.fascia);
            vec![v(a.x,front,top),v(a.x,a.y,top),v(a.x,a.y,a.z),v(a.x,b.y,a.z),v(a.x,b.y,band+self.rise),v(a.x,front,band)]
        };
        prism_faces(&profile,v(b.x-a.x,0.0,0.0))
    }
    fn hip(&self)->Vec<Vec<V3>>{
        let (a,b,top)=(self.min,self.max,self.max.z);
        let (front,band)=(a.y-self.overhang,top+self.fascia);
        let apex=v((a.x+b.x)*0.5,(front+b.y)*0.5,band+self.rise);
        let mut faces=if self.overhang<=0.0{self.walls([top;4])}else{
            let mut faces=self.eaves(band);
            faces.push(vec![v(a.x,front,top),v(b.x,front,top),v(b.x,front,band),v(a.x,front,band)]);
            faces.push(vec![v(a.x,b.y,a.z),v(b.x,b.y,a.z),v(b.x,b.y,band),v(a.x,b.y,band)]);
            faces
        };
        let eave=[v(a.x,front,band),v(b.x,front,band),v(b.x,b.y,band),v(a.x,b.y,band)];
        for k in 0..4{faces.push(vec![eave[k],eave[(k+1)%4],apex]);}
        faces
    }
}
/// Caras lógicas de un prisma recto: las dos tapas y un lateral por lado.
fn prism_faces(profile:&[V3],vector:V3)->Vec<Vec<V3>>{
    let n=profile.len();
    let mut faces=vec![profile.to_vec(),profile.iter().map(|q|*q+vector).collect()];
    for i in 0..n{let j=(i+1)%n;faces.push(vec![profile[i],profile[j],profile[j]+vector,profile[i]+vector]);}
    faces
}
