//! Crear: recetas, vista previa y cálculo versionado fuera del hilo de egui.
use std::{collections::{BTreeMap, BTreeSet}, sync::{atomic::{AtomicU64, Ordering}, mpsc, Arc}, time::Instant};
use egui::{ Rect, RichText, Sense, Stroke, Vec2};
use serde::{Deserialize, Serialize};
use vectorio_core::{generate::{self, Detail, GenContext, GenOutput, GeneratedGroup, ParamSpec, ParamValue, Params}, geom::Rng, p, BBox};
use crate::{app::{VectorioApp, ViewMode, PAPER_TINTS}, theme};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CreatePrefs { pub generator: String, pub params: BTreeMap<String, Params>, pub seed: u64, pub locks: BTreeSet<String> }
impl Default for CreatePrefs {
    fn default() -> Self { Self { generator: "blocks".into(), params: BTreeMap::new(), seed: 1, locks: BTreeSet::new() } }
}
#[derive(Clone, Debug, PartialEq)]
struct Request { generator: String, params: Params, context: GenContext }
struct Reply { version: u64, request: Request, result: Result<GenOutput, String>, seconds: f64 }
struct Worker { tx: mpsc::Sender<(u64, Request)>, rx: mpsc::Receiver<Reply>, version: Arc<AtomicU64> }
// Errores y partes del núcleo que se traducen al presentar, nunca al guardar recetas.
const _CORE_TEXTS: &[&str] = &[
    tm!("contorno"),tm!("tejados"),tm!("trama clara"),tm!("trama media"),tm!("trama oscura"),tm!("sombra"),
    tm!("Área o rotulador no válido"),tm!("Usa radios distintos para la hipotrocoide."),tm!("No se encuentra el generador"),
    tm!("El generador falló. Cambia los parámetros o prueba otra semilla."),
];
fn part_label(name: &str) -> String {
    if let Some(n) = name.strip_prefix("Capa ") { tr!("Capa %{n}", n=n) } else { crate::i18n::tr_key(name) }
}
fn protect(action: impl FnOnce() -> Result<GenOutput, String>) -> Result<GenOutput, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(action))
        .unwrap_or_else(|_| Err("El generador falló. Cambia los parámetros o prueba otra semilla.".into()))
}
fn calculate(request: &Request) -> Result<GenOutput, String> {
    protect(|| {
        generate::find(&request.generator).ok_or_else(|| "No se encuentra el generador".to_owned())?.generate(&request.params, &request.context)
    })
}
impl Worker {
    fn new(ctx: egui::Context) -> Self {
        let (tx, rx) = mpsc::channel::<(u64, Request)>(); let (out, input) = mpsc::channel();
        let version = Arc::new(AtomicU64::new(0)); let latest = version.clone();
        std::thread::spawn(move || {
            while let Ok(mut job) = rx.recv() {
                // Solo el pedido más reciente de la cola merece cálculo.
                for next in rx.try_iter() { job = next; }
                let (version, request) = job;
                if latest.load(Ordering::Relaxed) != version { continue; }
                let start = Instant::now(); let result = calculate(&request);
                if latest.load(Ordering::Relaxed) == version {
                    if out.send(Reply { version, request, result, seconds: start.elapsed().as_secs_f64() }).is_err() { break; }
                    ctx.request_repaint();
                }
            }
        });
        Self { tx, rx: input, version }
    }
}
#[derive(Default)]
pub struct CreateUi {
    settings: CreatePrefs,
    initialized: bool,
    seed_text: String,
    pub editing: Option<u64>,
    worker: Option<Worker>,
    version: u64,
    requested: Option<Request>,
    result: Option<Reply>,
    finalizing: bool,
}
impl CreateUi {
    pub fn edit(&mut self, group: &GeneratedGroup) {
        self.settings.generator = group.generator.clone(); self.settings.params.insert(group.generator.clone(), group.params.clone());
        self.settings.seed = group.seed; self.seed_text = group.seed.to_string(); self.settings.locks.clear();
        self.initialized = true; self.editing = Some(group.id); self.invalidate();
    }
    pub fn cancel(&mut self) { self.editing = None; self.invalidate(); }
    fn invalidate(&mut self) {
        self.version = self.version.wrapping_add(1);
        if let Some(worker) = &self.worker { worker.version.store(self.version, Ordering::Relaxed); }
        self.requested = None; self.result = None; self.finalizing = false;
    }
    fn randomize(&mut self) {
        let Some(generator) = generate::find(&self.settings.generator) else { return };
        let mut rng = Rng::new(self.settings.seed); self.settings.seed = rng.next_u64(); self.seed_text = self.settings.seed.to_string();
        generator.randomize(&mut rng, self.settings.params.entry(generator.id().into()).or_default(), &self.settings.locks);
        self.invalidate();
    }
    fn request(&mut self, ctx: &egui::Context, request: Request) {
        if self.requested.as_ref() == Some(&request) { return; }
        self.version = self.version.wrapping_add(1); self.result = None;
        let worker = self.worker.get_or_insert_with(|| Worker::new(ctx.clone()));
        worker.version.store(self.version, Ordering::Relaxed);
        if worker.tx.send((self.version, request.clone())).is_ok() { self.requested = Some(request); }
    }
    fn poll(&mut self) {
        if let Some(worker) = &self.worker {
            for reply in worker.rx.try_iter() {
                if reply.version == self.version && self.requested.as_ref() == Some(&reply.request) { self.result = Some(reply); }
            }
        }
    }
}
fn lock_button(ui: &mut egui::Ui, locked: &mut bool) {
    let pal=theme::current(ui.ctx());
    let (rect, response)=ui.allocate_exact_size(Vec2::splat(26.0),Sense::click());
    if response.clicked(){*locked = !*locked;}
    let color=if *locked {pal.accent} else {pal.secondary}; let c=rect.center();
    let stroke=Stroke::new(1.3f32,color);
    ui.painter().rect_stroke(Rect::from_center_size(c+egui::vec2(0.0,3.0),egui::vec2(10.0,8.0)),2.0,stroke);
    let shift=if *locked{0.0}else{3.0};
    ui.painter().add(egui::Shape::line(vec![c+egui::vec2(-3.5,-1.0),c+egui::vec2(-3.5,-5.0-shift),
        c+egui::vec2(3.5,-5.0-shift),c+egui::vec2(3.5,-1.0-shift)],stroke));
    response.on_hover_text(tr!("Bloquear este parámetro en Aleatorio"));
}
fn control(ui:&mut egui::Ui,spec:&ParamSpec,params:&mut Params,locks:&mut BTreeSet<String>)->bool {
    if !spec.shown(params) {return false;} let Some(id)=spec.id() else{return false};
    let mut changed=false;
    ui.horizontal(|ui|{
        ui.add(egui::Label::new(crate::i18n::tr_key(spec.label())).wrap());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
            let mut locked=locks.contains(id);lock_button(ui,&mut locked);
            if locked{locks.insert(id.into());}else{locks.remove(id);}
        });
    });
    let response=match spec {
        ParamSpec::Range{min,max,step,..}=>{
            let mut n=params.f64(id);
            let r=ui.add(egui::Slider::new(&mut n,*min..=*max).step_by(*step).max_decimals(if *step>=1.0{0}else{2}));
            changed=r.changed();if changed{params.set(id,ParamValue::Number(n));} r
        }
        ParamSpec::Choice{options,..}=>{
            let mut value=params.choice(id).to_owned();
            let r=egui::ComboBox::from_id_salt(("create-param",id)).width(ui.available_width())
                .selected_text(crate::i18n::tr_key(options.iter().find(|(v,_)|*v==value).map_or("",|(_,l)|*l)))
                .show_ui(ui,|ui|{for (v,label) in *options{changed|=ui.selectable_value(&mut value,(*v).into(),crate::i18n::tr_key(label)).changed();}}).response;
            if changed{params.set(id,ParamValue::Text(value));}r
        }
        ParamSpec::Toggle{..}=>{let mut value=params.flag(id);let r=ui.checkbox(&mut value,tr!("Activado"));changed=r.changed();
            if changed{params.set(id,ParamValue::Bool(value));}r}
        _=>return false,
    };
    if let Some(hint)=spec.hint(){response.on_hover_text(crate::i18n::tr_key(hint));}
    ui.add_space(4.0);changed
}
fn parameter_controls(ui:&mut egui::Ui,specs:&[ParamSpec],params:&mut Params,locks:&mut BTreeSet<String>)->bool {
    let mut changed=false;let mut start=0;
    while start<specs.len(){
        if let ParamSpec::Section{label}=specs[start]{
            let end=(start+1..specs.len()).find(|i|matches!(specs[*i],ParamSpec::Section{..})).unwrap_or(specs.len());
            egui::CollapsingHeader::new(crate::i18n::tr_key(label)).id_salt(("create-section",label)).default_open(true).show(ui,|ui|{
                for spec in &specs[start+1..end]{changed|=control(ui,spec,params,locks);}
            });start=end;
        }else{changed|=control(ui,&specs[start],params,locks);start+=1;}
    }changed
}
fn area(app:&VectorioApp,state:&CreateUi)->BBox {
    if let Some(group)=state.editing.and_then(|id|app.doc.generated_group(id)){return group.area;}
    let paper=&app.doc.paper;let margin=paper.margin_mm.max(0.0).min(paper.width_mm.min(paper.height_mm)*0.45);
    BBox{min:p(margin,margin),max:p(paper.width_mm-margin,paper.height_mm-margin)}
}
fn preview(ui:&mut egui::Ui,app:&VectorioApp,state:&CreateUi){
    let pal=theme::current(ui.ctx());theme::paint_canvas(ui.painter(),ui.max_rect(),&pal);
    let mut space=ui.max_rect().shrink(28.0);space.min.y=space.min.y.max(theme::TOOLBAR_H+theme::INSET+20.0);let paper=&app.doc.paper;
    let scale=(space.width()/paper.width_mm as f32).min(space.height()/paper.height_mm as f32).max(0.01);
    let size=egui::vec2(paper.width_mm as f32,paper.height_mm as f32)*scale;
    let sheet=Rect::from_center_size(space.center(),size);
    ui.painter().rect_filled(sheet.translate(egui::vec2(0.0,3.0)).expand(2.0),0.0,egui::Color32::from_black_alpha(24));
    ui.painter().rect_filled(sheet,0.0,theme::pen_color(PAPER_TINTS[app.prefs.paper_tint].1));
    let painter=ui.painter().with_clip_rect(sheet);
    if let Some(Reply{result:Ok(output),..})=&state.result{
        let group=state.editing.and_then(|id|app.doc.generated_group(id));
        for (i,part) in output.parts.iter().enumerate(){
            let existing=group.and_then(|g|g.parts.get(&part.name)).and_then(|id|app.doc.get(*id));
            if existing.is_some_and(|it|!it.visible){continue;}
            let suggested=if app.doc.pens.is_empty(){None}else{Some(if part.pen_order==Some(usize::MAX){app.doc.pens.len()-1}else{part.pen_order.unwrap_or(i)%app.doc.pens.len()})};
            let pen=existing.map(|it|it.stroke).unwrap_or_else(||group.and_then(|g|g.inks.get(&part.name).copied()).unwrap_or(suggested));
            let Some(pen)=pen.and_then(|i|app.doc.pens.get(i)) else{continue};
            for line in &part.lines{
                let points:Vec<_>=line.iter().map(|q|{
                    let q=if let Some(g)=group {
                        let place=existing.map(|it|it.place).or_else(||app.doc.group_items(g.id).first().and_then(|id|app.doc.get(*id)).map(|it|it.place)).unwrap_or(g.placement);
                        place.apply(*q-g.area.min)
                    }else{*q};
                    sheet.min+Vec2::new(q.x as f32,q.y as f32)*scale
                }).collect();
                painter.add(egui::Shape::line(points,Stroke::new((pen.width_mm as f32*scale).max(0.5),theme::pen_color(pen.color))));
            }
        }
    }else{painter.text(sheet.center(),egui::Align2::CENTER_CENTER,tr!("Preparando vista previa…"),egui::FontId::proportional(14.0),pal.secondary);}
}

pub fn show(app:&mut VectorioApp,ctx:&egui::Context){
    let mut state=std::mem::take(&mut app.create_ui);
    if !state.initialized{state.settings=app.prefs.create.clone();state.seed_text=state.settings.seed.to_string();state.initialized=true;}
    if generate::find(&state.settings.generator).is_none(){state.settings.generator="blocks".into();}
    let pal=theme::current(ctx);let mut changed=false;let mut place=false;let mut cancel=false;
    if !ctx.wants_keyboard_input() && ctx.input_mut(|i|i.consume_key(egui::Modifiers::NONE,egui::Key::N)){state.randomize();}
    // El área de controles reserva sus acciones: permanecen accesibles con ventanas bajas.
    egui::SidePanel::left("create-controls").exact_width(340.0).frame(egui::Frame::none()
        .inner_margin(egui::Margin{left:10.0,right:10.0,top:theme::TOOLBAR_H+theme::INSET,bottom:10.0})).show(ctx,|ui|{
        theme::paint_canvas(ui.painter(),ctx.screen_rect(),&pal);
        theme::glass_panel(ui,&pal,|ui|{
            ui.set_width(296.0);ui.set_height(ui.available_height());
            egui::TopBottomPanel::bottom("create-actions").frame(egui::Frame::none()).show_inside(ui,|ui|{
                ui.separator();
                if let Some(reply)=&state.result{
                    match &reply.result{
                        Ok(output)=>{
                            let total=output.total_m();
                            ui.label(RichText::new(tr!("%{meters} m · %{ms} ms",meters=crate::fmt::decimal(total,2),ms=crate::fmt::decimal(reply.seconds*1000.0,0))).strong());
                            egui::CollapsingHeader::new(tr!("Longitud por parte")).show(ui,|ui|{
                                for (part,meters) in output.parts.iter().zip(output.lengths_m()){
                                    ui.label(format!("{} · {} m",part_label(&part.name),crate::fmt::decimal(meters,2)));
                                }
                            });
                            if total>40.0{ui.colored_label(if total>80.0{egui::Color32::from_rgb(210,60,50)}else{pal.accent},
                                if total>80.0{tr!("El papel puede saturarse: más de 80 m.")}else{tr!("Más de 40 m: dibujo denso.")});}
                            if let Some(warning)=&output.warning{ui.label(crate::i18n::tr_key(warning));}
                        }
                        Err(error)=>{ui.colored_label(egui::Color32::from_rgb(210,60,50),crate::i18n::tr_key(error));}
                    }
                }else{ui.horizontal(|ui|{ui.spinner();ui.label(tr!("Calculando…"));});}
                let ready=state.result.as_ref().is_some_and(|r|r.result.is_ok())&&!state.finalizing&&state.seed_text.parse::<u64>().is_ok();
                ui.add_enabled_ui(ready,|ui|{place=theme::accent_button(ui,if state.editing.is_some(){ts!("Actualizar grupo")}else{ts!("Colocar en el lienzo")}).clicked();});
                if state.finalizing{ui.label(tr!("Preparando trazos finales…"));}
                cancel=ui.button(tr!("Cancelar")).clicked();
            });
            egui::CentralPanel::default().frame(egui::Frame::none()).show_inside(ui,|ui|{
                egui::ScrollArea::vertical().id_salt("create-settings").show(ui,|ui|{
                    ui.heading(tr!("Crear"));
                    theme::section(ui,ts!("GENERADORES"));
                    let mut category="";
                    for generator in generate::all(){
                        if generator.category()!=category{category=generator.category();ui.label(RichText::new(crate::i18n::tr_key(category)).strong());}
                        if ui.selectable_label(state.settings.generator==generator.id(),crate::i18n::tr_key(generator.name()))
                            .on_hover_text(crate::i18n::tr_key(generator.description())).clicked(){
                            state.settings.generator=generator.id().into();state.editing=None;changed=true;
                        }
                    }
                    let generator=generate::find(&state.settings.generator).unwrap();
                    ui.label(RichText::new(crate::i18n::tr_key(generator.description())).small().color(pal.secondary));
                    ui.add_space(10.0);
                    ui.horizontal(|ui|{ui.label(tr!("Semilla"));
                        if ui.add(egui::TextEdit::singleline(&mut state.seed_text).desired_width(145.0).char_limit(20)).changed(){
                            changed=true;if let Ok(seed)=state.seed_text.parse::<u64>(){state.settings.seed=seed;}
                        }
                        if ui.button(tr!("Aleatorio")).on_hover_text(tr!("N · nuevos parámetros y semilla")).clicked(){state.randomize();changed=true;}});
                    if state.seed_text.parse::<u64>().is_err(){ui.colored_label(pal.accent,tr!("Escribe una semilla entera entre 0 y 18446744073709551615."));}
                    let specs=generator.params();let params=state.settings.params.entry(generator.id().into()).or_default();
                    *params=params.normalized(&specs);
                    changed|=parameter_controls(ui,&specs,params,&mut state.settings.locks);
                });
            });
        });
    });
    if changed{state.invalidate();}
    if place{state.finalizing=true;state.requested=None;}
    if cancel{state.cancel();app.mode=ViewMode::Design;}
    if app.mode==ViewMode::Create{
        let generator=generate::find(&state.settings.generator).unwrap();
        let pen=app.doc.pens.get(app.active_pen).map_or(0.4,|p|p.width_mm);
        let request=Request{generator:generator.id().into(),params:state.settings.params.get(generator.id()).cloned().unwrap_or_default(),
            context:GenContext{area:area(app,&state),seed:state.settings.seed,pen_mm:pen,detail:if state.finalizing{Detail::Final}else{Detail::Preview}}};
        state.request(ctx,request);state.poll();
        if state.finalizing{
            if let Some(Reply{result:Ok(output),request,..})=&state.result{
                if state.editing.is_none_or(|id|app.doc.generated_group(id).is_some()){
                    let name=state.editing.and_then(|id|app.doc.generated_group(id)).map_or_else(||generator.name().into(),|g|g.name.clone());
                    let group=GeneratedGroup::new(name,generator.id().into(),request.params.clone(),request.context.seed,request.context.area);
                    let ids=app.doc.put_generated(group,output.clone(),state.editing);
                    app.set_selection(ids);app.tool=crate::app::Tool::Select;app.mode=ViewMode::Design;state.cancel();
                }else{state.result=None;state.cancel();app.mode=ViewMode::Design;}
            }else if state.result.is_some(){state.finalizing=false;}
        }
    }
    egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx,|ui|preview(ui,app,&state));
    if app.prefs.create!=state.settings{let prefs=crate::prefs::Preferences{create:state.settings.clone(),..app.prefs.clone()};app.set_prefs(ctx,prefs);}
    app.create_ui=state;
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(seed:u64)->Request { Request{generator:"spiro".into(),params:Params::default(),context:GenContext{
        area:BBox{min:p(10.0,10.0),max:p(200.0,287.0)},seed,pen_mm:0.4,detail:Detail::Preview}} }
    #[test]
    fn worker_discards_stale_results_and_cancel_invalidates_inflight_work(){
        let ctx=egui::Context::default();let mut ui=CreateUi::default();
        ui.request(&ctx,request(1));ui.request(&ctx,request(2));ui.request(&ctx,request(3));
        let start=Instant::now();
        while ui.result.is_none() && start.elapsed().as_secs_f64()<2.0 {ui.poll();std::thread::sleep(std::time::Duration::from_millis(1));}
        assert_eq!(ui.result.as_ref().unwrap().request.context.seed,3);
        ui.request(&ctx,request(4));ui.cancel();
        std::thread::sleep(std::time::Duration::from_millis(5));ui.poll();assert!(ui.result.is_none());
    }
    #[test]
    fn create_panel_handles_small_window_and_both_languages(){
        let _lock=crate::i18n::locale_lock();
        for lang in ["es","en"]{
            rust_i18n::set_locale(lang);let ctx=egui::Context::default();let mut app=VectorioApp::blank(&ctx);app.mode=ViewMode::Create;app.prefs.create.seed=u64::MAX;
            for _ in 0..3 {
                let input=egui::RawInput{screen_rect:Some(Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(980.0,620.0))),..Default::default()};
                let out=ctx.run(input,|ctx|show(&mut app,ctx));
                assert!(!out.shapes.is_empty());
                for shape in &out.shapes {
                    if let egui::Shape::Text(text) = &shape.shape {
                        if text.galley.text() == tr!("Crear") {
                            assert!(text.pos.y >= theme::TOOLBAR_H + theme::INSET, "El panel invade la barra");
                        }
                        if text.galley.text() == tr!("Cancelar") { assert!(text.pos.y < 620.0); }
                    }
                }
            }
            assert!(app.doc.generated.is_empty());assert_eq!(app.prefs.create.seed,u64::MAX);
        }
        rust_i18n::set_locale("es");
    }
    // La última red protege también un generador futuro que lance pánico.
    #[test]
    fn errors_are_reported_without_a_document_change(){
        let mut invalid=request(1);invalid.context.area.max=invalid.context.area.min;
        assert!(calculate(&invalid).is_err());invalid.generator="missing".into();assert!(calculate(&invalid).is_err());
        assert!(protect(||panic!("Generador de prueba roto")).is_err());
    }
}
