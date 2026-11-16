use super::*;
use crate::{p,BBox,scene::{Camera,PartId},generate::{GenContext,style::Style}};
fn scene(scale:f64)->(Scene,Style){viewed(scale,45.0)}
fn viewed(scale:f64,yaw_deg:f64)->(Scene,Style){
    let area=BBox{min:p(0.0,0.0),max:p(210.0,297.0)};
    let ctx=GenContext{area,seed:1,pen_mm:0.35,detail:Detail::Final};let style=Style::new(&ctx,0.8);
    let mut scene=Scene::new(Camera{scale,yaw_deg,..Default::default()},area);style.prepare(&mut scene);scene.set_sun(Some(v(-1.0,1.0,1.0)));(scene,style)
}
#[test]
fn pieces_are_deterministic_finite_and_skip_extreme_dimensions(){
    for piece in 0..12{let build=||{let(mut s,style)=scene(12.0);let mut rng=Rng::new(31);
        match piece{
            0..=3=>house(&mut s,style,v(0.0,0.0,0.0),v(3.0,3.0,3.0),[Roof::Flat,Roof::Shed,Roof::Gable,Roof::Hip][piece],&mut rng),
            4..=5=>bell_tower(&mut s,style,V3::default(),1.8,6.0,if piece==4{TowerTop::Dome}else{TowerTop::Spire}),
            6=>chimney(&mut s,style,V3::default(),v(1.0,1.0,2.0)),
            7=>stairs(&mut s,style,V3::default(),2.0,3.0,2.0),
            8=>retaining_wall(&mut s,style,V3::default(),v(3.0,1.0,2.0)),
            9=>terrain(&mut s,style,v(-2.0,-2.0,-2.0),v(2.0,2.0,0.0),&mut rng),
            _=>vegetation(&mut s,style,V3::default(),4.0,if piece==10{Plant::Cypress}else{Plant::Olive},&mut rng),
        }let out=style.finish(&s);assert!(out.total_m()>0.0);out};
        assert_eq!(build(),build());for part in build().parts{for line in part.lines{for q in line{assert!(q.x.is_finite()&&q.y.is_finite());}}}
    }
    let(mut s,style)=scene(10.0);let mut rng=Rng::new(1);
    for bad in [f64::NAN,f64::INFINITY,-1.0,0.0,1e20]{
        house(&mut s,style,V3::default(),v(bad,bad,bad),Roof::Gable,&mut rng);
        bell_tower(&mut s,style,v(bad,0.0,0.0),bad,bad,TowerTop::Dome);
        vegetation(&mut s,style,V3::default(),bad,Plant::Cypress,&mut rng);
    }
    assert_eq!(style.finish(&s).total_m(),0.0);
}
#[test]
fn atomic_windows_are_hidden_whole_and_small_details_are_omitted(){
    let(mut s,style)=scene(20.0);
    opening(&mut s,style,v(0.0,0.0,1.0),v(1.0,0.0,0.0),0.6,0.8,OpeningKind::Clear);
    let detail=s.part("detalle");assert_eq!(s.render().parts[detail.0 as usize].lines.len(),1);
    s.set_quiet(true);s.box3(v(0.2,-0.5,0.0),v(1.2,0.7,2.0));
    assert!(s.render().parts[detail.0 as usize].lines.is_empty());
    let(mut s,style)=scene(0.5);opening(&mut s,style,V3::default(),v(1.0,0.0,0.0),0.6,0.8,OpeningKind::Clear);
    assert_eq!(style.finish(&s).total_m(),0.0);
}
#[test]
fn audit_detects_slivers_and_overlapping_coplanar_faces(){
    let(mut s,_)=scene(20.0);s.box3(V3::default(),v(2.0,2.0,2.0));s.box3(v(1.0,1.0,0.0),v(3.0,3.0,2.0));
    assert!(!s.construction_issues(1.5).is_empty());
    let(mut s,_)=scene(20.0);s.box3(V3::default(),v(2.0,2.0,2.0));s.box3(v(1.0,1.0,0.0),v(3.0,3.0,2.02));
    assert!(s.construction_issues(1.5).iter().any(|s|s.contains("Rebanada")));
}

#[test]
fn a_chimney_and_an_eave_slab_are_boxes_with_nine_visible_edges(){
    let(mut s,style)=scene(20.0);chimney(&mut s,style,V3::default(),v(1.0,1.0,2.0));
    // Sin tramas: una caja aislada conserva las nueve aristas isométricas.
    let out=s.render();assert_eq!(out.parts[0].lines.len(),9);
    let(mut s,style)=scene(20.0);eave(&mut s,style,V3::default(),v(2.0,0.6,0.2));assert_eq!(s.render().parts[0].lines.len(),9);
    // Un canto de menos de 1,5 mm en la hoja no se dibuja.
    let(mut s,style)=scene(20.0);eave(&mut s,style,V3::default(),v(2.0,0.6,0.05));assert!(s.render().parts.iter().all(|p|p.lines.is_empty()));
    let(mut s,style)=scene(20.0);vegetation(&mut s,style,V3::default(),1.5,Plant::Shrub,&mut Rng::new(4));
    assert!(style.finish(&s).total_m()>0.0);
}
#[test]
fn openings_clear_wall_hatching_on_every_side_and_dark_ones_get_filled(){
    for kind in [OpeningKind::Clear,OpeningKind::Dark]{
        let(mut s,style)=scene(20.0);
        let faces=s.box3(V3::default(),v(4.0,4.0,3.0));
        let trama=s.part("trama");s.set_part(trama);
        for face in &faces{let n=crate::generate::style::normal(face);if n.z.abs()<1e-9{s.hatch_face(face,v(-n.y,n.x,0.0),0.5);}}
        // Misma dirección horizontal en fachadas opuestas, como hace la casa.
        let windows=[(v(1.0,0.0,1.0),v(1.0,0.0,0.0),v(0.0,-1.0,0.0)),(v(1.0,4.0,1.0),v(1.0,0.0,0.0),v(0.0,1.0,0.0)),
            (v(0.0,1.0,1.0),v(0.0,1.0,0.0),v(-1.0,0.0,0.0)),(v(4.0,1.0,1.0),v(0.0,1.0,0.0),v(1.0,0.0,0.0))];
        for (origin,h,_) in windows{opening(&mut s,style,origin,h,1.2,1.0,kind);}
        let out=s.render();let cam=*s.camera();
        let mut front=0;
        for (origin,h,outward) in windows{
            if !cam.facing(outward){continue;}front+=1;
            let poly:Vec<_>=[origin,origin+h*1.2,origin+h*1.2+v(0.0,0.0,1.0),origin+v(0.0,0.0,1.0)].iter().map(|q|cam.project(*q).0).collect();
            let inside=|q:crate::geom::P|{let c:Vec<f64>=(0..4).map(|i|{let (a,b)=(poly[i],poly[(i+1)%4]);(b.x-a.x)*(q.y-a.y)-(b.y-a.y)*(q.x-a.x)}).collect();
                c.iter().all(|x|*x>1e-6)||c.iter().all(|x|*x< -1e-6)};
            assert!(out.parts[trama.0 as usize].lines.iter().flat_map(|l|l.windows(2)).all(|w|!inside(w[0].lerp(w[1],0.5))),"{kind:?}");
        }
        assert_eq!(front,2);
        let detail=out.parts.iter().find(|p|p.name=="detalle").unwrap();
        // Dos marcos visibles; los oscuros añaden su relleno denso.
        if kind==OpeningKind::Clear{assert_eq!(detail.lines.len(),2);}else{assert!(detail.lines.len()>2*10,"{}",detail.lines.len());}
    }
}
/// Una prueba por pieza: cada una dibuja algo, repite el mismo dibujo con la misma
/// semilla, solo da coordenadas finitas y, sola, no deja avisos de construcción.
#[test]
fn every_kit_piece_draws_repeatably_and_cleanly_on_its_own(){
    type Piece=fn(&mut Scene,Style,&mut Rng);
    let pieces:[(&str,Piece);17]=[
        ("parapet",|s,st,_|parapet(s,st,V3::default(),v(3.0,3.0,0.6))),
        ("nave",|s,st,r|nave(s,st,V3::default(),v(6.0,3.0,4.0),r)),
        ("terrace",|s,st,r|{let (a,b)=(v(-3.0,-2.0,-2.0),v(3.0,2.0,0.0));let strata=Strata::new(s,st,a.z,b.z,a,b,r);terrace(s,st,a,b,Cut::block(),&strata)}),
        ("stair_head",|s,st,_|stair_head(s,st,V3::default(),v(1.6,1.3,2.1))),
        ("water_tank",|s,_,_|water_tank(s,V3::default(),0.35,0.8)),
        ("plinth",|s,st,r|{house(s,st,V3::default(),v(3.0,3.0,3.0),Roof::Flat,r);plinth(s,V3::default(),v(3.0,3.0,3.0),0.6);}),
        ("person",|s,_,_|person(s,V3::default(),1.7)),
        ("lamp",|s,_,_|lamp(s,V3::default(),3.0)),
        ("bench",|s,_,_|bench(s,V3::default(),true)),
        ("pot",|s,st,r|pot(s,st,V3::default(),0.5,r)),
        ("fountain",|s,st,_|fountain(s,st,V3::default(),1.2)),
        ("pergola",|s,_,_|pergola(s,V3::default(),v(3.0,2.0,0.0),2.1)),
        ("laundry",|s,_,r|laundry(s,V3::default(),v(3.0,0.0,0.0),1.8,r)),
        ("low_wall",|s,st,_|low_wall(s,st,V3::default(),v(4.0,3.0,0.0),1.0,Some((1.5,2.5)),[true;3])),
        ("balcony",|s,st,_|balcony(s,st,V3::default(),2.0,0.55)),
        ("furrows",|s,_,_|furrows(s,V3::default(),v(4.0,3.0,0.0),true,0.5)),
        ("paving",|s,_,_|paving(s,V3::default(),v(4.0,3.0,0.0),1.0)),
    ];
    for (name,piece) in pieces{
        let build=||{let(mut s,style)=scene(12.0);piece(&mut s,style,&mut Rng::new(7));
            assert!(s.construction_issues(1.5).is_empty(),"{name}: {:?}",s.construction_issues(1.5));style.finish(&s)};
        let out=build();assert!(out.total_m()>0.0,"{name} no dibuja nada");assert_eq!(out,build(),"{name}");
        assert!(out.parts.iter().flat_map(|p|&p.lines).flatten().all(|q|q.x.is_finite()&&q.y.is_finite()),"{name}");
    }
    // Las piezas nuevas descartan medidas imposibles sin dejar media geometría.
    let(mut s,style)=scene(12.0);
    for bad in [f64::NAN,f64::INFINITY,-1.0,0.0]{
        stair_head(&mut s,style,V3::default(),v(bad,1.0,2.0));water_tank(&mut s,V3::default(),bad,0.8);
        plinth(&mut s,V3::default(),v(3.0,3.0,3.0),bad);
    }
    assert_eq!(style.finish(&s).total_m(),0.0);
}
#[test]
fn no_kit_piece_draws_a_seam_between_coplanar_faces_of_its_own(){
    type Piece=fn(&mut Scene,Style,&mut Rng);
    let roofs=[Roof::Flat,Roof::Shed,Roof::Gable,Roof::Hip];
    let pieces:[(&str,Piece);20]=[
        ("casa plana",|s,st,r|house(s,st,V3::default(),v(4.0,3.0,3.0),Roof::Flat,r)),
        ("casa a un agua",|s,st,r|house(s,st,V3::default(),v(4.0,3.0,3.0),Roof::Shed,r)),
        ("casa a dos aguas",|s,st,r|house(s,st,V3::default(),v(4.0,3.0,3.0),Roof::Gable,r)),
        ("casa a cuatro aguas",|s,st,r|house(s,st,V3::default(),v(4.0,3.0,3.0),Roof::Hip,r)),
        ("trasera plana",|s,st,r|house_flush(s,st,V3::default(),v(4.0,3.0,3.0),Roof::Flat,r)),
        ("trasera a un agua",|s,st,r|house_flush(s,st,V3::default(),v(4.0,3.0,3.0),Roof::Shed,r)),
        ("trasera a dos aguas",|s,st,r|house_flush(s,st,V3::default(),v(4.0,3.0,3.0),Roof::Gable,r)),
        ("trasera a cuatro aguas",|s,st,r|house_flush(s,st,V3::default(),v(4.0,3.0,3.0),Roof::Hip,r)),
        ("nave",|s,st,r|nave(s,st,V3::default(),v(6.0,3.0,4.0),r)),
        ("pretil",|s,st,_|parapet(s,st,V3::default(),v(3.0,3.0,0.6))),
        ("escalera",|s,st,_|stairs(s,st,V3::default(),2.0,3.0,2.0)),
        ("tapia con puerta",|s,st,_|low_wall(s,st,V3::default(),v(4.0,3.0,0.0),1.0,Some((1.5,2.5)),[true;3])),
        ("tapia cerrada",|s,st,_|low_wall(s,st,V3::default(),v(4.0,3.0,0.0),1.0,None,[true;3])),
        ("tapia en U",|s,st,_|low_wall(s,st,V3::default(),v(4.0,3.0,0.0),1.0,None,[true,false,true])),
        ("tapia sin fondo con puerta",|s,st,_|low_wall(s,st,V3::default(),v(4.0,3.0,0.0),1.0,Some((1.5,2.5)),[false,false,true])),
        ("banco",|s,_,_|bench(s,V3::default(),true)),
        ("bancal",|s,st,r|{let (a,b)=(v(-3.0,-2.0,-2.0),v(3.0,2.0,0.0));let strata=Strata::new(s,st,a.z,b.z,a,b,r);terrace(s,st,a,b,Cut::block(),&strata)}),
        ("salida de escalera",|s,st,_|stair_head(s,st,V3::default(),v(1.6,1.3,2.1))),
        ("chimenea",|s,st,_|chimney(s,st,V3::default(),v(1.0,1.0,2.0))),
        ("pérgola",|s,_,_|pergola(s,V3::default(),v(3.0,2.0,0.0),2.1)),
    ];
    for scale in [8.0,12.0,20.0]{
        for (name,piece) in pieces{
            for seed in 0..6{
                let(mut s,style)=scene(scale);piece(&mut s,style,&mut Rng::new(seed));
                let seams=s.seams(&s.render());
                assert!(seams.is_empty(),"{name} a escala {scale}, semilla {seed}: {seams:?}");
                assert!(s.construction_issues(1.5).is_empty(),"{name}: {:?}",s.construction_issues(1.5));
            }
        }
    }
    // Las casas adosadas siguen mostrando su junta: son dos piezas.
    for roof in roofs{
        let(mut s,style)=scene(12.0);let mut rng=Rng::new(3);
        house_flush(&mut s,style,V3::default(),v(4.0,3.0,3.0),roof,&mut rng);
        house_flush(&mut s,style,v(4.0,0.0,0.0),v(8.0,3.0,3.0),roof,&mut rng);
        assert!(!s.seams(&s.render()).is_empty(),"{roof:?}: la junta entre casas adosadas desaparece");
    }
}
#[test]
fn the_bell_tower_stands_at_a_front_corner_and_its_meeting_with_the_nave_is_drawn(){
    for scale in [4.0,8.0,14.0]{
        for yaw in [225.0,315.0,45.0,135.0]{
            let mut added=Vec::new();
            for left in [false,true]{
                let plan=Church{width:5.0,length:12.5,height:6.0,tower:Some((left,TowerTop::Dome))};
                let draw=|meetings:bool|{
                    let(mut s,style)=viewed(scale,yaw);
                    s.set_intersections(meetings);
                    church(&mut s,style,v(0.0,0.0,0.0),plan,&mut Rng::new(3));
                    s
                };
                let s=draw(true);
                let issues=s.construction_issues(1.5);
                assert!(issues.is_empty(),"escala {scale}, vista {yaw}: {issues:?}");
                let with=s.render().length_m(PartId(0));
                added.push(with-draw(false).render().length_m(PartId(0)));
            }
            // En la esquina que queda delante, el campanario tapa su propio encuentro;
            // en la otra, el encuentro con el muro y el faldón se dibuja.
            assert!(added.iter().any(|a|*a>0.002),"escala {scale}, vista {yaw}: {added:?}");
        }
    }
}
