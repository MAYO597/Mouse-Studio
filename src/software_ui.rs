//! Editors for host-managed actions. All edits remain drafts until Save.
use crate::{
    model::{ActionKind, Binding, Profile, Settings},
    software,
    theme::*,
};
use eframe::egui::*;
#[derive(Default)]
pub struct Panels {
    pub advanced_open: bool,
    macro_open: bool,
    macro_target: (u64, usize, usize),
    macro_text: String,
    error: String,
    class_id: Option<u64>,
    class_snapshot: Vec<String>,
    class_text: String,
    gesture_start: Option<Pos2>,
    gesture_result: String,
    pub ring: crate::ring_ui::Editor,
}
pub enum Event {
    Test(Binding),
    Cancel,
}
pub fn binding(p: &Profile, button: usize, trigger: usize) -> &Binding {
    match trigger {
        1 => &p.advanced.hold[button],
        2 => &p.advanced.double[button],
        3 => &p.advanced.gestures[button],
        _ => &p.bindings[button],
    }
}
pub fn binding_mut(p: &mut Profile, button: usize, trigger: usize) -> &mut Binding {
    match trigger {
        1 => &mut p.advanced.hold[button],
        2 => &mut p.advanced.double[button],
        3 => &mut p.advanced.gestures[button],
        _ => &mut p.bindings[button],
    }
}
fn button_choice(ui: &mut Ui, id: &str, label: &str, selected: &mut Option<usize>, count: usize) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        ComboBox::from_id_salt(id)
            .selected_text(
                selected.map_or("Off", |i| crate::device::BUTTON_LABELS[i.min(count - 1)]),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(selected, None, "Off");
                for (i, label) in crate::device::BUTTON_LABELS[..count].iter().enumerate() {
                    ui.selectable_value(selected, Some(i), *label);
                }
            });
    });
}
impl Panels {
    pub fn close(&mut self) {
        self.advanced_open = false;
        self.macro_open = false;
        self.ring.open = false;
    }
    pub fn open_macro(&mut self, profile: u64, button: usize, trigger: usize, binding: &Binding) {
        self.macro_target = (profile, button, trigger);
        self.macro_text = software::macro_text(binding);
        self.error.clear();
        self.macro_open = true;
    }
    pub fn profile_editor(&mut self, ui: &mut Ui, settings: &mut Settings) {
        card().show(ui,|ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Application profiles").strong());
                toggle(ui,&mut settings.auto_switch,"Automatic application profiles");
            });
            muted(ui,"Match the focused app by its Hyprland class. The selected profile is the fallback.");
            let p=settings.active_mut();
            if self.class_id!=Some(p.id) || self.class_snapshot!=p.app_classes {
                self.class_id=Some(p.id);self.class_snapshot=p.app_classes.clone();self.class_text=p.app_classes.join(", ");
            }
            ui.label("Application classes for this profile (comma-separated)");
            if ui.add(TextEdit::singleline(&mut self.class_text).hint_text("Code, brave-browser, blender").margin(vec2(10.0,10.0)).desired_width(f32::INFINITY)).changed() {
                p.app_classes=self.class_text.split(',').map(str::trim).filter(|s|!s.is_empty()).map(str::to_string).collect();
                self.class_snapshot=p.app_classes.clone();
            }
        });
    }
    pub fn show(
        &mut self,
        ctx: &Context,
        settings: &mut Settings,
        can_test: bool,
        busy: bool,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        self.ring.show(ctx, settings);
        if self.advanced_open {
            let count = settings.device.button_count();
            Modal::new(Id::new("advanced-buttons")).show(ctx,|ui| {
                ui.set_width(580.0_f32.min(ctx.content_rect().width() - 48.0));ui.heading("Advanced button settings");
                muted(ui,"Changes stay in your draft until Save profiles. Each special mode needs a different button.");
                ScrollArea::vertical().id_salt("advanced-body").max_height((ctx.content_rect().height() - 180.0).clamp(100.0,560.0)).show(ui,|ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().slider_width = (ui.available_width() - 240.0).clamp(90.0,160.0);
                    let p=settings.active_mut();let a=&mut p.advanced;
                    ui.label(RichText::new("Click timing").strong());
                    ui.add(Slider::new(&mut a.hold_ms,150..=2000).text("Hold delay (ms)"));
                    ui.add(Slider::new(&mut a.double_ms,100..=600).text("Double-click window (ms)"));
                    muted(ui,"Hold actions suppress the click when held. Double-click delays a single click. Default Click keeps normal button behavior.");
                    ui.separator();
                    ui.label(RichText::new("Precision mode").strong());
                    button_choice(ui,"precision-button","Hold to slow the pointer",&mut a.precision_button,count);
                    ui.add(Slider::new(&mut a.precision_speed,-1.0..=1.0).text("Precision speed"));
                    muted(ui,"Uses flat acceleration while held, then restores the profile's pointer settings.");
                    ui.separator();
                    ui.label(RichText::new("Horizontal scrolling").strong());
                    button_choice(ui,"horizontal-button","Hold and turn the wheel",&mut a.horizontal_button,count);
                    muted(ui,"Save installs the required user helper. Wheel steps become real horizontal scroll events while held.");
                    ui.separator();
                    ui.label(RichText::new("Directional gestures").strong());
                    button_choice(ui,"gesture-button","Hold and move the pointer",&mut a.gesture_button,count);
                    ui.add(Slider::new(&mut a.gesture_threshold,10.0..=500.0).text("Threshold (pixels)"));
                    for (i,label) in ["Up","Down","Left","Right"].iter().enumerate() {
                        ui.push_id(i,|ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(*label);let b=&mut a.gestures[i];let old=b.kind;
                                ComboBox::from_id_salt("gesture-action").selected_text(b.kind.label()).show_ui(ui,|ui| {
                                    for kind in ActionKind::ALL {ui.selectable_value(&mut b.kind,kind,kind.label());}
                                });
                                if old!=b.kind { b.value.clear(); }
                                if b.kind==ActionKind::Macro {
                                    if ui.small_button("Edit macro").clicked() { self.open_macro(p.id,i,3,b); }
                                } else if !matches!(b.kind,ActionKind::Default|ActionKind::CycleDpi) {
                                    ui.add(TextEdit::singleline(&mut b.value).desired_width(170.0).hint_text("Action value"));
                                }
                            });
                        });
                    }
                    ui.add_space(8.0);ui.label("Gesture test pad · drag here; no actions execute");
                    let (rect,response)=ui.allocate_exact_size(vec2(ui.available_width(),110.0),Sense::drag());
                    ui.painter().rect_filled(rect,8,RAISED);
                    if response.drag_started(){self.gesture_start=response.interact_pointer_pos();}
                    if let Some(start)=self.gesture_start {
                        // The pointer can leave the window mid-drag; fall back to the start.
                        let end=ctx.pointer_latest_pos().unwrap_or(start);
                        ui.painter().line_segment([start,end],Stroke::new(2.0_f32,MINT));
                        if response.drag_stopped() {
                            self.gesture_result=software::direction(end.x-start.x,end.y-start.y,a.gesture_threshold)
                                .map_or("Below threshold",|i|["Up","Down","Left","Right"][i]).into();self.gesture_start=None;
                        }
                    }
                    ui.painter().text(rect.center(),Align2::CENTER_CENTER,&self.gesture_result,FontId::proportional(16.0),TEXT);
                });
                if ui.button("Done").clicked(){self.advanced_open=false;}
            });
        }
        if self.macro_open {
            Modal::new(Id::new("macro-editor")).show(ctx,|ui| {
                ui.set_width(520.0_f32.min(ctx.content_rect().width() - 48.0));ui.heading("Macro editor");
                ScrollArea::vertical().id_salt("macro-body").max_height((ctx.content_rect().height() - 140.0).clamp(100.0,560.0)).show(ui,|ui| {
                ui.set_width(ui.available_width());
                muted(ui,"One shortcut per line. Use wait 250 for a 250 ms delay. Maximum 64 steps and 60 seconds.");
                ui.add(TextEdit::multiline(&mut self.macro_text).desired_rows(10).desired_width(f32::INFINITY).hint_text("CTRL+C\nwait 250\nCTRL+V"));
                if !self.error.is_empty(){ui.colored_label(ERROR,&self.error);}
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(can_test && !busy,Button::new("Test macro")).clicked(){
                        match software::compose_macro(&self.macro_text) {
                            Ok(value)=>events.push(Event::Test(Binding {kind:ActionKind::Macro,value})),
                            Err(e)=>self.error=e,
                        }
                    }
                    if ui.add_enabled(can_test,Button::new("Cancel running macro")).clicked(){events.push(Event::Cancel);}
                });
                });
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Use macro").clicked(){
                        match software::compose_macro(&self.macro_text) {
                            Ok(value)=> {
                                if let Some(p)=settings.profiles.iter_mut().find(|p|p.id==self.macro_target.0) {
                                    *binding_mut(p,self.macro_target.1,self.macro_target.2)=Binding{kind:ActionKind::Macro,value};
                                }
                                self.macro_open=false;
                            },
                            Err(e)=>self.error=e,
                        }
                    }
                    if ui.button("Close editor").clicked(){self.macro_open=false;}
                });
            });
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) && !eframe::egui::Popup::is_any_open(ctx) {
            // Close only the topmost editor.
            if self.macro_open {
                self.macro_open = false;
            } else {
                self.advanced_open = false;
            }
        }
        events
    }
}
