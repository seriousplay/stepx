//! Opt-in UI driver. Only dispatches pointer/keyboard events into painted controls.
//! No send, confirm, upload, or executor action shortcuts. State export is read-only.
use super::widgets::El;
use super::*;
use facloud_ui::TypedActionView;
thread_local! { static IDS:RefCell<std::collections::BTreeSet<String>>=RefCell::new(Default::default()); static LAST:RefCell<(u64,Option<Instant>,bool)>=RefCell::new((0,None,false)); }
pub(super) fn control(id: String, el: El) -> El {
    if std::env::var_os("STEPX_SCENARIO_DIR").is_none() {
        return el;
    }
    IDS.with(|s| s.borrow_mut().insert(id.clone()));
    SavePosition::new(el, &id).for_single_frame().finish()
}
fn events(ctx: &mut ViewContext<Companion>, events: Vec<Event>) {
    let id = ctx.window_id();
    let sent = std::cell::Cell::new(false);
    ctx.on_frame_drawn(move |ctx, drawn| {
        if id != drawn || sent.replace(true) {
            return;
        }
        let w = ctx.windows().platform_window(id).unwrap();
        for event in &events {
            (w.callbacks().event_callback)(event.clone(), ctx);
        }
    });
}
impl Companion {
    pub(super) fn scenario_tick(&mut self, ctx: &mut ViewContext<Self>) {
        let dir = PathBuf::from(std::env::var_os("STEPX_SCENARIO_DIR").unwrap());
        LAST.with(|cell| {
   let mut last=cell.borrow_mut();if last.1.is_some_and(|t|t.elapsed()<Duration::from_millis(300)){return;}last.1=Some(Instant::now());
   if !last.2 && !self.initializing && !self.busy {
     let serial=std::env::var("STEPX_WORKSPACE_TEST_SERIAL").expect("explicit test phone");
     if self.selected.as_ref().is_none_or(|d|d.serial!=serial) {self.handle_action(&Action::Select(serial),ctx);return;}
     self.handle_action(&Action::Page(Page::Amoo),ctx);last.2=true;
   }
   let mut names=IDS.with(|s|s.borrow().clone());
   for name in ["composer-editor","composer-send","composer-surface","workspace-intent-anchor","context-choice-0","context-choice-1","context-choice-2","context-choice-3"]{names.insert(name.into());}
   let regions=names.into_iter().filter_map(|id|ctx.element_position_by_id(id.clone()).map(|r|(id,[r.origin().x(),r.origin().y(),r.width(),r.height()]))).collect::<std::collections::BTreeMap<_,_>>();
   let state=serde_json::json!({"pid":std::process::id(),"seq":last.0,"ready":last.2,"status":self.status,"busy":self.busy,"running":self.workspace_busy(),"error":self.workspace.error,"draft":self.fields[F_AMOO].text,"attachments":self.amoo_attachments.iter().map(|a|a.local_path.as_ref().map(|p|p.to_string_lossy())).collect::<Vec<_>>(),"mode":format!("{:?}",self.push.shell_mode),"intent":format!("{:?}",self.workspace.intent),"proposal":self.workspace.proposal.as_ref().map(|p|&p.text),"task":self.workspace_task(),"work_status":self.work.status,"lab_status":self.studio_lab.status,"viewport":[self.workspace.viewport.x(),self.workspace.viewport.y()],"regions":regions});
   let _=std::fs::write(dir.join("state.tmp"),state.to_string());let _=std::fs::rename(dir.join("state.tmp"),dir.join("state.json"));
   let Ok(bytes)=std::fs::read(dir.join("command.json"))else{return;};let Ok(v)=serde_json::from_slice::<serde_json::Value>(&bytes)else{return;};let seq=v["seq"].as_u64().unwrap_or(0);if seq<=last.0{return;}last.0=seq;
   let mut error=None;
   match v["op"].as_str().unwrap_or("") {
    "click"=>{let id=v["id"].as_str().unwrap_or("");if let Some(r)=ctx.element_position_by_id(id.to_owned()) {let position=r.origin()+r.size()/2.; events(ctx,vec![Event::MouseMoved{position,cmd:false,shift:false,is_synthetic:false},Event::LeftMouseDown{position,modifiers:Default::default(),click_count:1,is_first_mouse:false},Event::LeftMouseUp{position,modifiers:Default::default()}]);}else{error=Some(format!("painted target missing: {id}"));}},
    "drop"=>events(ctx,vec![Event::DragAndDropFiles { paths:v["paths"].as_array().unwrap().iter().map(|p|p.as_str().unwrap().to_owned()).collect(),location:ctx.element_position_by_id("composer-editor".to_owned()).unwrap().origin()+Vector2F::new(20.,20.) }]),
    "type"=>events(ctx,vec![Event::TypedCharacters{chars:v["text"].as_str().unwrap_or("").into()}]),
    "key"=>events(ctx,vec![Event::KeyDown{keystroke:facloud_ui::keymap::Keystroke{key:v["key"].as_str().unwrap_or("enter").into(),shift:v["shift"].as_bool().unwrap_or(false),ctrl:v["ctrl"].as_bool().unwrap_or(false),..Default::default()},chars:String::new(),details:Default::default(),is_composing:false}]),
    "resize"=>{let w=facloud_ui::windowing::winit::winit_window_for_id(ctx,ctx.window_id()).unwrap();let _=w.request_inner_size(winit::dpi::LogicalSize::new(v["w"].as_f64().unwrap_or(1100.),v["h"].as_f64().unwrap_or(760.)));},
    "capture"=>{let path=dir.join(v["name"].as_str().unwrap_or("ui.png"));ctx.windows().platform_window(ctx.window_id()).unwrap().as_ctx().request_frame_capture(Box::new(move|f|{image::save_buffer(path,&f.data,f.width,f.height,image::ColorType::Rgba8).unwrap();}));},
    "quit"=>ctx.close_window(),
    _=>error=Some("unknown UI command".into()),
   }
   let _=std::fs::write(dir.join("ack.json"),serde_json::json!({"seq":seq,"error":error}).to_string());ctx.notify();
  });
    }
}
