//! Crear: recetas, vista previa y cálculo versionado fuera del hilo de egui.
use std::{collections::{BTreeMap, BTreeSet}, sync::{atomic::{AtomicU64, Ordering}, mpsc, Arc}, time::Instant};
use egui::{ Rect, RichText, Sense, Stroke, Vec2};
use serde::{Deserialize, Serialize};
use vectorio_core::{generate::{self, Detail, GenContext, GenOutput, GeneratedGroup, Generator, InkChoice, ParamSpec, ParamValue, Params}, geom::Rng, p, BBox};
use crate::{app::{VectorioApp, ViewMode, PAPER_TINTS}, theme};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CreatePrefs {
    pub generator: String, pub params: BTreeMap<String, Params>, pub seed: u64, pub locks: BTreeSet<String>,
    /// Juego de tintas por generador. Ausente: el juego propio del generador;
    /// `None`: reparte los rotuladores del documento, como las recetas antiguas.
    pub inks: BTreeMap<String, Option<InkChoice>>,
}
impl Default for CreatePrefs {
    fn default() -> Self { Self { generator: "blocks".into(), params: BTreeMap::new(), seed: 1, locks: BTreeSet::new(), inks: BTreeMap::new() } }
}
impl CreatePrefs {
    fn ink_choice(&self, generator: &dyn Generator) -> Option<InkChoice> {
        let plan = generator.inks();
        self.inks.get(generator.id()).cloned().unwrap_or_else(|| Some(plan.default_choice())).map(|c| plan.normalized(&c))
    }
}
#[derive(Clone, Debug, PartialEq)]
struct Request { generator: String, params: Params, context: GenContext }
struct Reply { version: u64, request: Request, result: Result<GenOutput, String>, seconds: f64 }
struct Worker { tx: mpsc::Sender<(u64, Request)>, rx: mpsc::Receiver<Reply>, version: Arc<AtomicU64> }
// Errores y partes del núcleo que se traducen al presentar, nunca al guardar recetas.
const _CORE_TEXTS: &[&str] = &[
    tm!("contorno"),tm!("tejados"),tm!("trama clara"),tm!("trama media"),tm!("trama oscura"),tm!("sombra"),
    tm!("Intensidad de tinta"),tm!("Megastructuras"),tm!("Puerto y ciudadela"),tm!("Costa viva"),
    tm!("detalle"),tm!("trama"),tm!("vegetación"),tm!("terreno"),tm!("línea"),tm!("acento"),
    tm!("Bolígrafo"),tm!("Cal y añil"),tm!("Tierra y olivo"),tm!("Tinta china"),tm!("Atardecer"),tm!("Vectorio"),tm!("Océano"),tm!("Brasas"),tm!("Cartel"),tm!("Cal y teja"),tm!("Abismo"),tm!("Órbita"),tm!("Fiesta"),
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
    view: crate::vectorize_ui::PreviewView,
    remembered: Option<CreatePrefs>,
    last_preview: Option<GenOutput>,
    initialized: bool,
    seed_text: String,
    pub editing: Option<u64>,
    worker: Option<Worker>,
    version: u64,
    requested: Option<Request>,
    result: Option<Reply>,
    finalizing: bool,
    thumbnails: ThumbnailCache,
}
impl CreateUi {
    pub fn load_preferences(&mut self, prefs:&CreatePrefs) {
        if !self.initialized { self.settings=prefs.clone();self.seed_text=self.settings.seed.to_string();self.initialized=true; }
    }
    pub fn edit(&mut self, group: &GeneratedGroup) {
        if self.remembered.is_none() { self.remembered = Some(self.settings.clone()); }
        self.settings.generator = group.generator.clone(); self.settings.params.insert(group.generator.clone(), group.params.clone());
        self.settings.seed = group.seed; self.seed_text = group.seed.to_string();
        self.settings.inks.insert(group.generator.clone(), group.ink_choice.clone());
        self.last_preview=None;self.initialized = true; self.editing = Some(group.id); self.invalidate();
    }
    pub fn cancel(&mut self) {
        if let Some(settings) = self.remembered.take() { self.settings = settings; self.seed_text = self.settings.seed.to_string(); }
        self.editing = None; self.last_preview = None; self.invalidate();
    }
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
                if reply.version == self.version && self.requested.as_ref() == Some(&reply.request) {
                    if let Ok(output) = &reply.result { self.last_preview = Some(output.clone()); }
                    self.result = Some(reply);
                }
            }
        }
    }
}
// Las miniaturas tienen su propio trabajador: nunca se encolan delante del
// Preview/Final principal. Se calculan bajo demanda, una vez por variante y sesión.
type ThumbnailReply=(String,Result<GenOutput,String>);
type ThumbnailRequest=(String,Request);
struct ThumbnailWorker { tx:mpsc::Sender<ThumbnailRequest>, rx:mpsc::Receiver<ThumbnailReply> }
#[derive(Default)]
struct ThumbnailCache {
    worker:Option<ThumbnailWorker>,
    requested:BTreeSet<String>,
    outputs:BTreeMap<String,GenOutput>,
}
impl ThumbnailCache {
    fn poll(&mut self){if let Some(worker)=&self.worker{for (id,result) in worker.rx.try_iter(){if let Ok(out)=result{self.outputs.insert(id,out);}}}}
    fn request(&mut self,ctx:&egui::Context,choice:&scene_catalog::Choice){
        let key=choice.key();
        if !self.requested.insert(key.clone()){return;}
        let worker=self.worker.get_or_insert_with(||{
            let (tx,rx)=mpsc::channel::<ThumbnailRequest>();let (out,input)=mpsc::channel();let ctx=ctx.clone();
            std::thread::spawn(move||{while let Ok((id,request))=rx.recv(){
                if out.send((id,calculate(&request))).is_err(){break;}ctx.request_repaint();
            }});ThumbnailWorker{tx,rx:input}
        });
        let request=Request{generator:choice.generator.into(),params:choice.params(),context:GenContext{area:BBox{min:p(3.0,3.0),max:p(97.0,67.0)},seed:0,pen_mm:0.35,detail:Detail::Preview}};
        let _=worker.tx.send((key,request));
    }
}
fn thumbnail(ui:&mut egui::Ui,output:&GenOutput,generator:&dyn Generator)->egui::Response{
    let (rect,response)=ui.allocate_exact_size(egui::vec2(190.0,133.0),Sense::click());
    ui.painter().rect_filled(rect,4.0,egui::Color32::WHITE);
    let painter=ui.painter().with_clip_rect(rect);
    let plan=generator.inks();let choice=plan.default_choice();
    for (i,part) in output.parts.iter().enumerate(){
        let color=theme::pen_color(plan.ink_for(&choice,i,&part.name).color);
        for line in &part.lines {painter.add(egui::Shape::line(line.iter().map(|q|rect.min+egui::vec2(q.x as f32,q.y as f32)*1.9).collect(),Stroke::new(0.6f32,color)));}
    }
    response.widget_info(||egui::WidgetInfo::labeled(egui::WidgetType::Button,ui.is_enabled(),crate::i18n::tr_key(generator.name())));
    theme::focus_ring(ui,&response);
    response
}
fn lock_button(ui: &mut egui::Ui, locked: &mut bool, parameter: &str) {
    let pal=theme::current(ui.ctx());
    let (rect, response)=ui.allocate_exact_size(Vec2::splat(26.0),Sense::click());
    if response.clicked(){*locked = !*locked;}
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *locked,
        tr!("Bloquear %{param} en Aleatorio", param = parameter)));
    theme::focus_ring(ui, &response);
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
            let mut locked=locks.contains(id);lock_button(ui,&mut locked,&crate::i18n::tr_key(spec.label()));
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
/// Juego de tintas y número de rotuladores. No recalcula la escena: solo cambia
/// los colores, salvo que el grosor de la trama cambie la separación mínima.
fn ink_controls(ui:&mut egui::Ui,generator:&dyn Generator,settings:&mut CreatePrefs){
    let pal=theme::current(ui.ctx());let plan=generator.inks();
    let mut choice=settings.ink_choice(generator);let before=choice.clone();
    ui.add_space(6.0);
    ui.horizontal(|ui|{
        ui.label(tr!("Juego de tintas"));
        let selected=choice.as_ref().and_then(|c|plan.palette(&c.palette)).map_or_else(||tr!("Rotuladores del documento"),|p|crate::i18n::tr_key(p.name));
        egui::ComboBox::from_id_salt("create-inks").width(ui.available_width()).selected_text(selected).show_ui(ui,|ui|{
            for palette in plan.palettes{
                let on=choice.as_ref().is_some_and(|c|c.palette==palette.id);
                if ui.selectable_label(on,crate::i18n::tr_key(palette.name)).clicked()&&!on{
                    let count=choice.as_ref().map_or(plan.default_count,|c|c.count);
                    choice=Some(plan.normalized(&InkChoice{palette:palette.id.into(),count}));
                }
            }
            ui.separator();
            if ui.selectable_label(choice.is_none(),tr!("Rotuladores del documento")).on_hover_text(tr!("Reparte las tintas que ya tiene el documento")).clicked(){choice=None;}
        });
    });
    if let Some(c)=&mut choice{
        let len=plan.palette(&c.palette).map_or(1,|p|p.inks.len());
        ui.horizontal(|ui|{
            ui.label(tr!("Rotuladores"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                for n in (1..=len).rev(){
                    if ui.selectable_label(c.count==n,n.to_string()).clicked(){c.count=n;}
                }
            });
        });
        let cell_width=(ui.available_width()-10.0)*0.5;
        egui::Grid::new("create-ink-legend").num_columns(2).spacing(egui::vec2(8.0,2.0)).show(ui,|ui|{
            for (i,ink) in plan.active(c).iter().enumerate(){
                ui.allocate_ui_with_layout(egui::vec2(cell_width,20.0),egui::Layout::left_to_right(egui::Align::Center),|ui|{
                    let role=if plan.cycle{String::new()}else{format!(" · {}",crate::i18n::tr_key(ink.role))};
                    theme::swatch(ui,ink.color,13.0).on_hover_text(format!("{} · {} mm{role}",ink.name,crate::fmt::decimal(ink.width_mm,1)));
                    ui.add(egui::Label::new(RichText::new(ink.name).small().color(pal.secondary)).wrap());
                });
                if i%2==1{ui.end_row();}
            }
        });
        ui.label(RichText::new(tr!("Se añaden al documento al colocar, reutilizando los del mismo color.")).small().color(pal.secondary));
    }
    if choice!=before{settings.inks.insert(generator.id().into(),choice);}
    ui.add_space(6.0);
}
const PANEL_WIDTH: f32 = 360.0;

fn preview(ui:&mut egui::Ui,app:&VectorioApp,state:&mut CreateUi){
    let pal=theme::current(ui.ctx());theme::paint_canvas(ui.painter(),ui.max_rect(),&pal);
    let mut space=ui.max_rect().shrink(theme::INSET);
    space.min.x += PANEL_WIDTH + theme::INSET;
    space.min.y=space.min.y.max(theme::TOOLBAR_H+theme::INSET+50.0);
    let paper=&app.doc.paper;
    let source=egui::vec2(paper.width_mm as f32,paper.height_mm as f32);
    let response=ui.interact(space,ui.id().with("create-navigation"),Sense::click_and_drag());
    let (zoom,scroll,touch)=ui.input(|i|(i.zoom_delta(),i.smooth_scroll_delta,i.multi_touch()));
    if let Some(pos)=response.hover_pos(){
        if zoom!=1.0{state.view.zoom_at(zoom,pos,space,source);}
        if let Some(touch)=touch{state.view.pan(touch.translation_delta,space,source);}
        else if zoom==1.0 && scroll!=Vec2::ZERO{state.view.pan(scroll,space,source);}
    }
    if response.dragged() && touch.is_none(){
        state.view.pan(response.drag_delta(),space,source);
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }else if response.hovered(){ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);}
    if response.double_clicked(){state.view=Default::default();}
    let sheet=state.view.image_rect(space,source);
    let scale=sheet.width()/source.x;
    theme::set_backdrop(ui.ctx(),Some((sheet,theme::pen_color(PAPER_TINTS[app.prefs.paper_tint].1))));
    egui::Area::new(egui::Id::new("create-view-controls")).enabled(!app.document_modal()).fixed_pos(egui::pos2(space.left(),theme::TOOLBAR_H+theme::INSET)).show(ui.ctx(),|ui|{
        theme::glass_panel_with(ui,&pal,theme::CONTROL_RADIUS,egui::Margin::symmetric(10.0,6.0),theme::Material::Panel,|ui|{
            ui.horizontal(|ui|{
                if ui.button(tr!("Alejar")).clicked(){state.view.zoom_at(0.8,space.center(),space,source);}
                ui.label(format!("{:.0} %",state.view.zoom*100.0));
                if ui.button(tr!("Acercar")).clicked(){state.view.zoom_at(1.25,space.center(),space,source);}
                if ui.button(tr!("Ajustar")).clicked(){state.view=Default::default();}
            });
        });
    });
    ui.painter().rect_filled(sheet.translate(egui::vec2(0.0,3.0)).expand(2.0),0.0,egui::Color32::from_black_alpha(24));
    ui.painter().rect_filled(sheet,0.0,theme::pen_color(PAPER_TINTS[app.prefs.paper_tint].1));
    let painter=ui.painter().with_clip_rect(sheet.intersect(ui.max_rect()));
    let current = state.result.as_ref().and_then(|r|r.result.as_ref().ok());
    if let Some(output)=current.or(state.last_preview.as_ref()){
        let group=state.editing.and_then(|id|app.doc.generated_group(id));
        let generator=generate::find(&state.settings.generator);
        let choice=generator.and_then(|g|state.settings.ink_choice(g));
        for (i,part) in output.parts.iter().enumerate(){
            let existing=group.and_then(|g|g.parts.get(&part.name)).and_then(|id|app.doc.get(*id));
            if existing.is_some_and(|it|!it.visible){continue;}
            // Con el mismo juego, una parte ya colocada conserva la tinta que tenga en el documento.
            let keep=group.is_some_and(|g|g.ink_choice==choice&&g.inks.contains_key(&part.name));
            let ink=match (generator,&choice) {
                (Some(generator),Some(choice)) if !keep=>{let ink=generator.inks().ink_for(choice,i,&part.name);Some((ink.color,ink.width_mm))}
                _=>{
                    let suggested=if app.doc.pens.is_empty(){None}else{Some(if part.pen_order==Some(usize::MAX){app.doc.pens.len()-1}else{part.pen_order.unwrap_or(i)%app.doc.pens.len()})};
                    let pen=existing.map(|it|it.stroke).unwrap_or_else(||group.and_then(|g|g.inks.get(&part.name).copied()).unwrap_or(suggested));
                    pen.and_then(|i|app.doc.pens.get(i)).map(|pen|(pen.color,pen.width_mm))
                }
            };
            let Some((color,width_mm))=ink else{continue};
            for line in &part.lines{
                let points:Vec<_>=line.iter().map(|q|{
                    let q=if let Some(g)=group {
                        let place=existing.map(|it|it.place).or_else(||app.doc.group_items(g.id).first().and_then(|id|app.doc.get(*id)).map(|it|it.place)).unwrap_or(g.placement);
                        place.apply(*q-g.area.min)
                    }else{*q};
                    sheet.min+Vec2::new(q.x as f32,q.y as f32)*scale
                }).collect();
                painter.add(egui::Shape::line(points,Stroke::new((width_mm as f32*scale).max(0.5),if current.is_none() && !state.finalizing { theme::pen_color(color).gamma_multiply(0.35) } else { theme::pen_color(color) })));
            }
        }
    }
    if let Some(Reply { result: Err(error), .. }) = &state.result {
        painter.text(sheet.center(),egui::Align2::CENTER_CENTER,crate::i18n::tr_key(error),egui::FontId::proportional(14.0),pal.text);
    } else if current.is_none() && state.last_preview.is_none() {
        painter.text(sheet.center(),egui::Align2::CENTER_CENTER,tr!("Preparando vista previa…"),egui::FontId::proportional(14.0),pal.secondary);
    }
}

pub fn show(app:&mut VectorioApp,ctx:&egui::Context){
    let mut state=std::mem::take(&mut app.create_ui);
    state.load_preferences(&app.prefs.create);state.thumbnails.poll();
    if generate::find(&state.settings.generator).is_none(){state.settings.generator="blocks".into();}
    let pal=theme::current(ctx);let mut changed=false;let mut place=false;let mut cancel=false;let mut volume=false;
    if !app.document_modal() && !ctx.wants_keyboard_input() && ctx.input_mut(|i|i.consume_key(egui::Modifiers::NONE,egui::Key::N)){state.randomize();}
    // El área de controles reserva sus acciones: permanecen accesibles con ventanas bajas.
    egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx,|ui|{if app.document_modal() { ui.disable(); }preview(ui,app,&mut state)});
    let top=theme::TOOLBAR_H+theme::INSET;
    let height=(ctx.screen_rect().height()-top-theme::INSET).max(120.0);
    egui::Area::new(egui::Id::new("create-controls")).enabled(!app.document_modal()).fixed_pos(egui::pos2(theme::INSET,top)).constrain(false).default_size(egui::vec2(PANEL_WIDTH,height)).order(egui::Order::Middle).show(ctx,|ui|{
        theme::glass_panel(ui,&pal,|ui|{
            let width=PANEL_WIDTH-2.0*theme::PANEL_PAD;
            ui.set_width(width);
            ui.set_max_width(width);
            ui.set_height(height-2.0*theme::PANEL_PAD);
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
                if generate::find(&state.settings.generator).is_some_and(|g|g.has_volume()){
                    volume=ui.add_enabled(ready,egui::Button::new(tr!("Abrir en Volumen"))).clicked();
                }
                if state.finalizing{ui.label(tr!("Preparando trazos finales…"));}
                cancel=ui.button(tr!("Cancelar")).clicked();
            });
            egui::CentralPanel::default().frame(egui::Frame::none()).show_inside(ui,|ui|{
                egui::ScrollArea::vertical().id_salt("create-settings").auto_shrink([false,false]).show(ui,|ui|{
                    ui.set_max_width(ui.available_width());
                    ui.spacing_mut().slider_width=(ui.available_width()-80.0).max(100.0);
                    changed|=scene_catalog::show(ui,&mut state);
                    let generator=generate::find(&state.settings.generator).unwrap();
                    ui.add_space(10.0);
                    ui.horizontal(|ui|{ui.label(tr!("Semilla"));
                        if ui.add(egui::TextEdit::singleline(&mut state.seed_text).desired_width(145.0).char_limit(20)).changed(){
                            changed=true;if let Ok(seed)=state.seed_text.parse::<u64>(){state.settings.seed=seed;}
                        }
                        if ui.button(tr!("Aleatorio")).on_hover_text(tr!("N · nuevos parámetros y semilla")).clicked(){state.randomize();changed=true;}});
                    if state.seed_text.parse::<u64>().is_err(){ui.colored_label(pal.accent,tr!("Escribe una semilla entera entre 0 y 18446744073709551615."));}
                    ink_controls(ui,generator,&mut state.settings);
                    let specs=generator.params();let params=state.settings.params.entry(generator.id().into()).or_default();
                    *params=params.normalized(&specs);
                    let variant=scene_catalog::variant_parameter(generator.id());
                    let controls=specs.into_iter().filter(|spec|variant.is_none()||spec.id()!=variant).collect::<Vec<_>>();
                    changed|=parameter_controls(ui,&controls,params,&mut state.settings.locks);
                });
            });
        });
    });
    if changed{state.invalidate();}
    if volume {
        if let Some(reply)=&state.result{
            app.volumetric_ui.open_scene(ctx,reply.request.generator.clone(),reply.request.params.clone(),vectorio_core::generate::GenContext{detail:Detail::Final,..reply.request.context.clone()},app.doc.paper.clone(),app.doc.pens.clone());
            app.mode=ViewMode::Volumetric;
        }
    }
    if place{state.finalizing=true;state.requested=None;}
    if cancel{state.cancel();app.mode=ViewMode::Design;}
    if app.mode==ViewMode::Create{
        let generator=generate::find(&state.settings.generator).unwrap();
        // La separación mínima de trama depende del rotulador que la dibuja.
        let pen=match state.settings.ink_choice(generator){
            Some(choice)=>generator.inks().ink_for(&choice,0,"trama").width_mm,
            None=>app.doc.pens.get(app.active_pen).map_or(0.4,|p|p.width_mm),
        };
        let request=Request{generator:generator.id().into(),params:state.settings.params.get(generator.id()).cloned().unwrap_or_default(),
            context:GenContext{area:area(app,&state),seed:state.settings.seed,pen_mm:pen,detail:if state.finalizing{Detail::Final}else{Detail::Preview}}};
        state.request(ctx,request);state.poll();
        if state.finalizing{
            if let Some(Reply{result:Ok(output),request,..})=&state.result{
                if state.editing.is_none_or(|id|app.doc.generated_group(id).is_some()){
                    let name=state.editing.and_then(|id|app.doc.generated_group(id)).map_or_else(||generator.name().into(),|g|g.name.clone());
                    let mut group=GeneratedGroup::new(name,generator.id().into(),request.params.clone(),request.context.seed,request.context.area);
                    group.ink_choice=state.settings.ink_choice(generator);
                    let ids=match &group.ink_choice{
                        Some(choice)=>{let inks=generator.inks().assign(choice,output.parts.iter().map(|p|p.name.as_str()));
                            app.doc.put_generated_with_inks(group,output.clone(),state.editing,&inks)}
                        None=>app.doc.put_generated(group,output.clone(),state.editing),
                    };
                    app.set_selection(ids);app.tool=crate::app::Tool::Select;app.mode=ViewMode::Design;state.cancel();
                }else{state.result=None;state.cancel();app.mode=ViewMode::Design;}
            }else if state.result.is_some(){state.finalizing=false;}
        }
    }

    if state.editing.is_none() && state.remembered.is_none() && app.prefs.create!=state.settings{let prefs=crate::prefs::Preferences{create:state.settings.clone(),..app.prefs.clone()};app.set_prefs(ctx,prefs);}
    app.create_ui=state;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameter_locks_expose_a_named_checkbox_and_its_state() {
        let _locale = crate::i18n::locale_lock();
        rust_i18n::set_locale("es");
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut locked = true;
        let output = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| lock_button(ui, &mut locked, "Vista"));
        });
        let tree = output.platform_output.accesskit_update.unwrap();
        let (_, node) = tree.nodes.iter().find(|(_, node)| node.name() == Some("Bloquear Vista en Aleatorio")).unwrap();
        assert_eq!(format!("{:?}", node.role()), "CheckBox");
        assert_eq!(format!("{:?}", node.toggled()), "Some(True)");
    }
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
            app.prefs.create.generator="generador-obsoleto".into();
            for _ in 0..3 {
                let input=egui::RawInput{screen_rect:Some(Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(980.0,620.0))),..Default::default()};
                let out=ctx.run(input,|ctx|show(&mut app,ctx));
                assert!(generate::find(&app.create_ui.settings.generator).is_some());
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
    #[test]
    fn create_pinch_is_local_to_the_drawing_and_does_not_change_the_recipe(){
        let ctx=egui::Context::default();let mut app=VectorioApp::blank(&ctx);app.mode=ViewMode::Create;
        let before=app.prefs.create.clone();
        let mut frame=|time,events|{
            let _=ctx.run(egui::RawInput{screen_rect:Some(Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(980.0,620.0))),time:Some(time),events,..Default::default()},|ctx|show(&mut app,ctx));
            app.create_ui.view.zoom
        };
        frame(0.0,vec![]);
        frame(0.1,vec![egui::Event::PointerMoved(egui::pos2(700.0,350.0))]);
        assert_eq!(frame(0.2,vec![egui::Event::Zoom(2.0)]),2.0);
        frame(0.3,vec![egui::Event::PointerMoved(egui::pos2(180.0,300.0))]);
        assert_eq!(frame(0.4,vec![egui::Event::Zoom(2.0)]),2.0);
        assert_eq!(app.prefs.create.seed,before.seed);
    }

    #[test]
    fn editing_restores_remembered_parameters_and_locks_after_cancel_or_update(){
        let prefs=CreatePrefs{generator:"spiro".into(),seed:99,locks:BTreeSet::from(["layers".into()]),..Default::default()};
        let mut state=CreateUi::default();state.load_preferences(&prefs);
        let group=GeneratedGroup::new("Grupo".into(),"white_village".into(),Params::default(),123,request(0).context.area);
        state.edit(&group);state.randomize();state.settings.locks.insert("church".into());
        assert_eq!(state.remembered.as_ref(),Some(&prefs));state.cancel();assert_eq!(state.settings,prefs);
        state.edit(&group);state.settings.seed=456;state.cancel();assert_eq!(state.settings,prefs);
    }
    #[test]
    fn final_keeps_last_preview_and_error_is_drawn_on_the_sheet(){
        let _lock=crate::i18n::locale_lock();let ctx=egui::Context::default();let app=VectorioApp::blank(&ctx);
        let mut state=CreateUi::default();let r=request(1);let out=calculate(&r).unwrap();
        state.last_preview=Some(out.clone());state.finalizing=true;state.request(&ctx,r.clone());
        assert_eq!(state.last_preview,Some(out));
        state.result=Some(Reply{version:state.version,request:r,result:Err("No se encuentra el generador".into()),seconds:0.0});
        let output=ctx.run(egui::RawInput::default(),|ctx|{egui::CentralPanel::default().show(ctx,|ui|preview(ui,&app,&mut state));});
        assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()==tr!("No se encuentra el generador"))));
        assert!(!output.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()==tr!("Preparando vista previa…"))));
    }
    #[test]
    fn thumbnails_are_computed_once_per_requested_variant_on_their_own_worker(){
        let ctx=egui::Context::default();let mut cache=ThumbnailCache::default();
        let choices=scene_catalog::choices();
        let choices=choices.iter().filter(|c|c.generator=="spiro").collect::<Vec<_>>();
        assert_eq!(choices.len(),2);
        assert!(cache.worker.is_none());
        for choice in &choices{cache.request(&ctx,choice);cache.request(&ctx,choice);}
        let start=Instant::now();
        while cache.outputs.len()<2 && start.elapsed().as_secs_f64()<5.0{cache.poll();std::thread::sleep(std::time::Duration::from_millis(2));}
        assert_eq!(cache.requested.len(),2);assert_eq!(cache.outputs.len(),2);
        assert_ne!(cache.outputs[&choices[0].key()].parts,cache.outputs[&choices[1].key()].parts);
    }

    #[test]
    fn errors_are_reported_without_a_document_change(){
        let mut invalid=request(1);invalid.context.area.max=invalid.context.area.min;
        assert!(calculate(&invalid).is_err());invalid.generator="missing".into();assert!(calculate(&invalid).is_err());
        assert!(protect(||panic!("Generador de prueba roto")).is_err());
    }
}

#[cfg(test)]
#[path = "create_render_qa.rs"]
mod render_qa;

#[path = "create_catalog.rs"]
mod scene_catalog;
