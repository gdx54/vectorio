use super::*;
use crate::{p, Document, Placement};
fn context(seed:u64)->GenContext{GenContext{area:BBox{min:p(15.0,15.0),max:p(195.0,282.0)},seed,pen_mm:0.4,detail:Detail::Final}}
#[test]
fn expressive_materials_gain_colour_without_losing_plotter_limits(){
    for id in ["white_village","festival_city","harbour","megastructure"]{
        let generator=find(id).unwrap();let ctx=context(4);
        let rich=generator.generate(&Params::default(),&ctx).unwrap();
        let mut params=Params::default();params.set("richness",ParamValue::Number(0.0));
        let light=generator.generate(&params,&ctx).unwrap();
        assert!(rich.total_m()>light.total_m()*1.05,"{id}: {} / {}",rich.total_m(),light.total_m());
        assert!(rich.total_m()<80.0,"{id}");
        let plan=generator.inks();let choice=plan.default_choice();
        let roof=rich.parts.iter().position(|p|p.name=="tejados"&&!p.lines.is_empty()).expect(id);
        let shadow=rich.parts.iter().position(|p|p.name=="sombra"&&!p.lines.is_empty()).expect(id);
        assert_ne!(plan.ink_for(&choice,roof,"tejados").color,plan.ink_for(&choice,shadow,"sombra").color,"{id}");
    }
}

#[test]
fn architectural_recipes_cover_views_modes_papers_and_pen_widths(){
    for (id,key,modes) in [("megastructure","structure",["tower","court","portal"]),("harbour","port",["fishing","industrial","citadel"])]{
        let generator=find(id).unwrap();
        for seed in 0..12 {
            let mut params=Params::default();generator.randomize(&mut Rng::new(seed),&mut params,&BTreeSet::new());
            params.set(key,ParamValue::Text(modes[seed as usize%3].into()));
            params.set("yaw",ParamValue::Text(["225","315","45","135"][seed as usize%4].into()));
            params.set("richness",ParamValue::Number(if seed%2==0{0.0}else{1.5}));
            let mut ctx=context(seed);ctx.pen_mm=if seed%2==0{0.25}else{0.7};
            if seed%3==0{ctx.area=BBox{min:p(20.0,5.0),max:p(285.0,195.0)};}
            let out=generator.generate(&params,&ctx).unwrap();
            assert!(out.total_m()>1.0&&out.total_m()<80.0,"{id} {seed}: {}",out.total_m());
            for q in out.parts.iter().flat_map(|p|&p.lines).flatten(){assert!(q.x.is_finite()&&q.y.is_finite()&&ctx.area.contains(*q,1e-7),"{id} {seed} {q:?}");}
            let mut document=Document::default();let mut group=GeneratedGroup::new(id.into(),id.into(),params,seed,ctx.area);
            group.ink_choice=Some(generator.inks().default_choice());
            let inks=generator.inks().assign(group.ink_choice.as_ref().unwrap(),out.parts.iter().map(|p|p.name.as_str()));
            document.put_generated_with_inks(group,out,None,&inks);
            let back=Document::from_json(&document.to_json()).unwrap();
            assert_eq!(back.generated[0].generator,id);assert_eq!(back.generated[0].seed,seed);
            assert_eq!(back.generated[0].parts,document.generated[0].parts);
            assert_eq!(back.generated[0].ink_choice,document.generated[0].ink_choice);
            assert_eq!(back.pens,document.pens);assert_eq!(back.items.len(),document.items.len());
            for (a,b) in back.items.iter().zip(&document.items){
                assert_eq!((a.id,a.stroke,a.group,a.place),(b.id,b.stroke,b.group,b.place));
                let (crate::doc::Geometry::Path{lines:a,..},crate::doc::Geometry::Path{lines:b,..})=(&a.geometry,&b.geometry) else{panic!("geometría inesperada")};
                assert_eq!(a.len(),b.len());for (a,b) in a.iter().zip(b){assert_eq!(a.len(),b.len());for (p,q) in a.iter().zip(b){assert!(p.dist(*q)<1e-9);}}
            }
        }
    }
}
#[test]
fn params_migrate_validate_show_and_respect_locks(){
    let generator=find("blocks").unwrap();let specs=generator.params();
    let defaults=Params::default().normalized(&specs);assert_eq!(defaults.int("rows"),4);
    let mut params=defaults.clone();params.set("rows",ParamValue::Number(100.0));params.set("unknown",ParamValue::Bool(true));
    params.set("yaw",ParamValue::Text("invalid".into()));params.set("street",ParamValue::Number(f64::NAN));
    let mut params=params.normalized(&specs);assert_eq!(params.int("rows"),8);assert!(!params.0.contains_key("unknown"));
    assert_eq!(params.choice("yaw"),"45");assert_eq!(params.f64("street"),0.6);
    generator.randomize(&mut Rng::new(4),&mut params,&BTreeSet::from(["rows".into()]));assert_eq!(params.int("rows"),8);
    let show=find("spiro").unwrap().params().into_iter().find(|s|s.id()==Some("phase")).unwrap();
    let mut p=Params::default();p.set("layers",ParamValue::Number(1.0));assert!(!show.shown(&p));
    p.set("layers",ParamValue::Number(4.0));assert!(show.shown(&p));
    assert_eq!(serde_json::from_str::<Params>(&serde_json::to_string(&params).unwrap()).unwrap(),params);
}
#[test]
fn generators_are_deterministic_and_curves_are_closed(){
    for generator in all(){
        let p=Params::default();let out=generator.generate(&p,&context(13)).unwrap();
        assert_eq!(out,generator.generate(&p,&context(13)).unwrap());
        if generator.id()=="spiro"{for part in out.parts{assert_eq!(part.lines.len(),1);assert_eq!(part.lines[0].first(),part.lines[0].last());}}
    }
}
#[test]
#[ignore="500 semillas por generador; medir en release con --ignored --nocapture"]
fn randomized_500_seeds_per_generator(){
    for generator in all(){let mut worst=std::time::Duration::ZERO;
        for seed in 0..500{
            let mut params=Params::default();generator.randomize(&mut Rng::new(seed),&mut params,&BTreeSet::new());
            // Terreno y encuadre no se sortean: se alternan para cubrir llano y maqueta.
            if generator.id()=="white_village"{
                params.set("relief",ParamValue::Text(if seed%2==0{"flat"}else{"terraces"}.into()));
                params.set("frame",ParamValue::Text(if seed%3==0{"model"}else{"crop"}.into()));
            }
            let ctx=context(seed);let start=std::time::Instant::now();let output=generator.generate(&params,&ctx).unwrap();
            worst=worst.max(start.elapsed());
            if matches!(generator.id(),"white_village"|"festival_city"|"domes") {assert!(output.warning.is_none(),"{} semilla {seed}: {:?}",generator.id(),output.warning);}
            assert!(output.total_m()<80.0,"{} semilla {seed}: {} m",generator.id(),output.total_m());
            for part in output.parts{for line in part.lines{for point in line{
                assert!(point.x.is_finite()&&point.y.is_finite());assert!(ctx.area.contains(point,1e-7),"{point:?}");
            }}}
        }
        eprintln!("{}: 500 semillas; peor tiempo {:?}",generator.name(),worst);assert!(worst.as_secs_f64()<1.0);
    }
}
fn recipe()->GeneratedGroup{GeneratedGroup::new("Prueba".into(),"spiro".into(),Params::default(),1,context(1).area)}
fn output(names:&[&str])->GenOutput{GenOutput{parts:names.iter().map(|name|GenPart{name:(*name).into(),lines:vec![vec![p(15.0,15.0),p(30.0,20.0)]],pen_order:None}).collect(),warning:None}}
#[test]
fn old_document_and_generated_recipe_roundtrip(){
    let d=Document::default();let json=d.to_json();assert!(!json.contains("generated"));assert_eq!(Document::from_json(&json).unwrap(),d);
    let mut d=d;d.put_generated(recipe(),output(&["a","b"]),None);
    assert_eq!(Document::from_json(&d.to_json()).unwrap(),d);
}
#[test]
fn regeneration_keeps_identity_inks_placement_layers_and_restores_missing_part(){
    let mut d=Document::default();let ids=d.put_generated(recipe(),output(&["a","b"]),None);let group=d.generated[0].id;
    let transform=Placement{pos:p(60.0,80.0),scale:1.7,rotation_deg:32.0};
    for id in &ids{d.get_mut(*id).unwrap().place=transform;}
    {let it=d.get_mut(ids[0]).unwrap();it.stroke=Some(2);it.locked=true;it.visible=false;}
    d.items.reverse();let old=d.items.clone();
    let mut r=recipe();r.seed=99;
    d.put_generated(r.clone(),output(&["a","b","c"]),Some(group));
    assert_eq!(&d.items[..2],&old[..]);assert_eq!(d.get(*d.generated[0].parts.get("c").unwrap()).unwrap().place,transform);
    d.put_generated(r.clone(),output(&["b"]),Some(group));assert!(d.get(ids[0]).is_none());
    let saved=d.to_json();let mut d=Document::from_json(&saved).unwrap();
    d.put_generated(r,output(&["a","b"]),Some(group));let a=d.generated[0].parts["a"];
    assert_eq!(d.get(a).unwrap().stroke,Some(2));assert_eq!(d.get(a).unwrap().place,transform);
    let copies=d.duplicate(&d.group_items(group),p(8.0,8.0));assert_eq!(copies.len(),2);
    assert_eq!(d.generated.len(),2);assert_ne!(d.get(copies[0]).unwrap().group,Some(group));
}

#[test]
fn orphan_recipes_are_pruned_only_from_saved_document(){
    let mut d=Document::default();let ids=d.put_generated(recipe(),output(&["a"]),None);
    d.items.retain(|it|!ids.contains(&it.id));assert_eq!(d.generated.len(),1);
    assert!(Document::from_json(&d.to_json()).unwrap().generated.is_empty());
}
#[test]
fn randomization_with_invalid_steps_stays_finite_and_default_scenes_fill_the_area(){
    for step in [0.0,-1.0,f64::NAN,f64::INFINITY]{let specs=[ParamSpec::Range{id:"x",label:"x",min:0.0,max:10.0,step,default:1.0,
        random:Some((0.0,10.0)),show:None,hint:None}];let mut p=Params::default();randomize_specs(&specs,&mut Rng::new(1),&mut p,&BTreeSet::new());assert!(p.f64("x").is_finite());}
    for id in ["blocks","white_village"]{let out=find(id).unwrap().generate(&Params::default(),&context(0)).unwrap();
        let bounds=BBox::of(out.parts.iter().flat_map(|p|&p.lines));assert!(bounds.width()/context(0).area.width()>0.9||bounds.height()/context(0).area.height()>0.9,"{id}: {bounds:?}");}
}

#[test]
fn phase_two_blocks_recipe_keeps_its_version_and_parts_on_load(){
    let generator=find("blocks").unwrap();let mut params=Params::default().normalized(&generator.params());
    params.set("scene_version",ParamValue::Number(2.0));
    let before=generator.generate(&params,&context(7)).unwrap();
    let mut d=Document::default();let mut old=params.clone();old.0.remove("scene_version");old.0.remove("sun_height");
    let group=GeneratedGroup::new("Antiguo".into(),"blocks".into(),old,7,context(7).area);d.put_generated(group,before.clone(),None);
    let back=Document::from_json(&d.to_json()).unwrap();
    assert_eq!(back.generated[0].params.f64("scene_version"),2.0);
    assert_eq!(generator.generate(&back.generated[0].params,&context(7)).unwrap(),before);
}

#[test]
#[ignore="Tiempos de Pueblo blanco de serie en release"]
fn benchmark_village_default_preview_and_final(){
    let generator=find("white_village").unwrap();
    for detail in [Detail::Preview,Detail::Final]{let mut ctx=context(0);ctx.detail=detail;let mut times=Vec::new();
        for _ in 0..7{let t=std::time::Instant::now();let out=generator.generate(&Params::default(),&ctx).unwrap();
            times.push(t.elapsed());assert!(out.total_m()<40.0);assert!(out.warning.is_none());std::hint::black_box(out);}
        times.sort();eprintln!("Pueblo blanco serie {detail:?}: mediana {:?}, máximo {:?}",times[3],times[6]);
        assert!(times[3].as_millis()<if detail==Detail::Preview{100}else{300});
    }
}
#[test]
fn ink_sets_reuse_or_add_pens_and_respect_user_changes(){
    let plan=find("white_village").unwrap().inks();
    let names=["contorno","trama","vegetación","terreno"];
    let mut d=Document::default();let before=d.pens.len();
    let choice=InkChoice{palette:"india_ink".into(),count:3};
    let mut r=recipe();r.ink_choice=Some(choice.clone());
    let ids=d.put_generated_with_inks(r.clone(),output(&names),None,&plan.assign(&choice,names));
    // «Negro fino» ya existe: se reutiliza. Verde gris y gris medio se añaden una vez.
    assert_eq!(d.pens.len(),before+2);assert_eq!(d.get(ids[0]).unwrap().stroke,Some(0));
    assert_eq!(d.get(ids[1]).unwrap().stroke,d.get(ids[3]).unwrap().stroke,"terreno usa la trama con 3 rotuladores");
    assert!(d.pens.iter().any(|p|p.name=="Verde gris"&&p.custom_name));
    let group=d.generated[0].id;assert_eq!(d.generated[0].ink_choice,Some(choice.clone()));
    // Mismo juego: la tinta cambiada por el usuario se conserva; la parte nueva recibe la del juego.
    d.get_mut(ids[2]).unwrap().stroke=Some(1);
    let names5=["contorno","trama","vegetación","terreno","sombra"];
    let ids=d.put_generated_with_inks(r.clone(),output(&names5),Some(group),&plan.assign(&choice,names5));
    assert_eq!(d.get(ids[2]).unwrap().stroke,Some(1));assert_eq!(d.get(ids[4]).unwrap().stroke,d.get(ids[1]).unwrap().stroke);
    // Juego distinto: todas las partes toman sus tintas nuevas, sin duplicar rotuladores al repetir.
    let other=InkChoice{palette:"lime_indigo".into(),count:4};r.ink_choice=Some(other.clone());
    let ids=d.put_generated_with_inks(r.clone(),output(&names5),Some(group),&plan.assign(&other,names5));
    let colors:Vec<_>=ids.iter().map(|id|d.pens[d.get(*id).unwrap().stroke.unwrap()].color).collect();
    assert_eq!(colors,[[43,47,107],[90,123,181],[94,112,52],[181,128,58],[90,123,181]]);
    let count=d.pens.len();d.put_generated_with_inks(r,output(&names5),Some(group),&plan.assign(&other,names5));assert_eq!(d.pens.len(),count);
    let back=Document::from_json(&d.to_json()).unwrap();assert_eq!(back,d);
    // Las recetas antiguas no tienen juego y siguen cargando.
    assert!(!Document::default().to_json().contains("ink_choice"));
    let mut old=Document::default();old.put_generated(recipe(),output(&["a"]),None);assert!(!old.to_json().contains("ink_choice"));
}
#[test]
fn recipe_version_is_hidden_never_randomized_and_defaults_to_current(){
    for (id,current) in [("blocks",3.0),("white_village",1.0)]{
        let generator=find(id).unwrap();let specs=generator.params();
        let spec=specs.iter().find(|s|s.id()==Some(SCENE_VERSION)).unwrap();
        assert!(matches!(spec,ParamSpec::Version{..})&&!spec.shown(&Params::default())&&spec.hint().is_none());
        // Una receta nueva nace con la versión vigente; una fuera de rango se acota.
        let mut params=Params::default().normalized(&specs);assert_eq!(params.f64(SCENE_VERSION),current,"{id}");
        for (raw,want) in [(0.0,1.0),(99.0,current),(f64::NAN,current)]{
            params.set(SCENE_VERSION,ParamValue::Number(raw));assert_eq!(params.normalized(&specs).f64(SCENE_VERSION),want,"{id} {raw}");
        }
        // Sortear nunca toca la versión, ni siquiera sin candados.
        params.set(SCENE_VERSION,ParamValue::Number(1.0));
        for seed in 0..20{generator.randomize(&mut Rng::new(seed),&mut params,&BTreeSet::new());assert_eq!(params.f64(SCENE_VERSION),1.0,"{id}");}
    }
}
#[test]
fn flat_village_and_bleed_frame_cover_the_sheet_without_construction_warnings(){
    let generator=find("white_village").unwrap();
    for (seed,relief,country) in [(1,"flat",0.0),(2,"flat",1.0),(3,"terraces",1.0),(4,"terraces",0.0)]{
        let mut params=Params::default();params.set("relief",ParamValue::Text(relief.into()));params.set("country",ParamValue::Number(country));
        let ctx=context(seed);let out=generator.generate(&params,&ctx).unwrap();
        assert!(out.warning.is_none(),"{relief} {seed}: {:?}",out.warning);
        // A sangre el terreno llega a los cuatro bordes del área.
        let bounds=BBox::of(out.parts.iter().flat_map(|p|&p.lines));
        for (a,b) in [(bounds.min.x,ctx.area.min.x),(bounds.min.y,ctx.area.min.y),(bounds.max.x,ctx.area.max.x),(bounds.max.y,ctx.area.max.y)]{
            assert!((a-b).abs()<0.5,"{relief} {seed}: {bounds:?}");
        }
    }
}
/// Maqueta y Manzanas se encuadran sin tocar el borde: medio margen libre como mínimo
/// en un área cuadrada, una vertical y una apaisada.
fn framed_outputs_keep_half_margin(seeds:std::ops::Range<u64>){
    let areas=[BBox{min:p(10.0,10.0),max:p(190.0,190.0)},BBox{min:p(15.0,15.0),max:p(195.0,282.0)},BBox{min:p(5.0,20.0),max:p(290.0,200.0)}];
    let mut closest=f64::INFINITY;
    for id in ["white_village","blocks"]{let generator=find(id).unwrap();
        for seed in seeds.clone(){for area in areas{
            let mut params=Params::default();generator.randomize(&mut Rng::new(seed),&mut params,&BTreeSet::new());
            if id=="white_village"{params.set("frame",ParamValue::Text("model".into()));}
            let ctx=GenContext{area,seed,pen_mm:0.35,detail:Detail::Final};let out=generator.generate(&params,&ctx).unwrap();
            let b=BBox::of(out.parts.iter().flat_map(|p|&p.lines));let margin=1.0f64.min(area.width().min(area.height())*0.02);
            let gap=(b.min.x-area.min.x).min(b.min.y-area.min.y).min(area.max.x-b.max.x).min(area.max.y-b.max.y);
            closest=closest.min(gap);
            assert!(gap>=margin*0.5-1e-6,"{id} semilla {seed} {area:?}: {gap:.3} mm del borde");
        }}
    }
    eprintln!("encuadre: distancia mínima al borde {closest:.3} mm");
}
#[test]
fn model_frame_never_touches_the_border(){framed_outputs_keep_half_margin(0..3);}
#[test]
#[ignore="500 semillas por área y generador; en release con --ignored --nocapture"]
fn model_frame_never_touches_the_border_500_seeds(){framed_outputs_keep_half_margin(0..500);}
