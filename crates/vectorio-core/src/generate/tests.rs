use super::*;
use crate::{p, Document, Placement};
fn context(seed:u64)->GenContext{GenContext{area:BBox{min:p(15.0,15.0),max:p(195.0,282.0)},seed,pen_mm:0.4,detail:Detail::Final}}
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
            let ctx=context(seed);let start=std::time::Instant::now();let output=generator.generate(&params,&ctx).unwrap();
            worst=worst.max(start.elapsed());assert!(output.total_m()<80.0,"{} semilla {seed}: {} m",generator.id(),output.total_m());
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
