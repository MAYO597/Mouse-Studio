use super::{
    HAPTIC_LEVELS, HapticSettings, HardwareSettings, Snapshot, ThumbWheelRemap, WheelMode,
};
use crate::model::{ActionKind, Binding};
use crate::theme::*;
use eframe::egui::*;

pub fn dpi(ui: &mut Ui, h: &mut HardwareSettings, s: &Snapshot) {
    card().show(ui,|ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new("Sensor DPI").strong());
        muted(ui,s.dpi.map_or("Live DPI unavailable".into(),|d|format!("Live: {d} DPI")));
        let supported=!s.available||s.dpi.is_some();
        if !supported {muted(ui,"This mouse does not expose adjustable sensor DPI.");}
        let mut managed=h.dpi.is_some();
        if ui.add_enabled(supported||managed,Checkbox::new(&mut managed,"Manage sensor DPI")).changed() {h.dpi=managed.then_some(s.dpi.unwrap_or(1600));}
        if let Some(dpi)=&mut h.dpi {
            ui.horizontal_wrapped(|ui| {
                ui.label("DPI");
                ui.add(DragValue::new(dpi).range(100..=32000).speed(50));
                if !s.dpi_values.is_empty() {
                    ComboBox::from_id_salt("sensor-dpi").selected_text("Supported values").show_ui(ui,|ui| {
                        for d in &s.dpi_values {ui.selectable_value(dpi,*d,format!("{d}"));}
                    });
                }
            });
            if !s.dpi_values.is_empty()&&!s.dpi_values.contains(dpi) {ui.colored_label(ERROR,"Choose a supported sensor DPI before saving.");}
            ui.horizontal_wrapped(|ui| {
                for preset in &h.presets {
                    if ui.selectable_label(*dpi==preset.dpi,format!("{} · {}",preset.name,preset.dpi)).clicked() {*dpi=preset.dpi;}
                }
            });
        }
        CollapsingHeader::new("Edit DPI presets").show(ui,|ui| {
            let mut remove=None;
            let removable=h.presets.len()>1;
            for (i,p) in h.presets.iter_mut().enumerate() {
                ui.push_id(i,|ui| {ui.horizontal_wrapped(|ui| {
                    ui.add(TextEdit::singleline(&mut p.name).desired_width(110.0));
                    ui.add(DragValue::new(&mut p.dpi).range(100..=32000).speed(50));
                    if ui.add_enabled(removable,Button::new("Remove")).clicked(){remove=Some(i);}
                });});
            }
            if let Some(i)=remove {h.presets.remove(i);}
            if ui.add_enabled(h.presets.len()<8,Button::new("Add DPI preset")).clicked() {
                let dpi=(100..=32000).step_by(100).find(|v|!h.presets.iter().any(|p|p.dpi==*v)).unwrap_or(100);
                h.presets.push(super::DpiPreset{name:format!("Preset {}",h.presets.len()+1),dpi});
            }
            muted(ui,"Cycle DPI advances through these presets in order. Assign it to a button, then Save.");
        });
        muted(ui,"Save profiles applies the chosen sensor DPI. Turn management off to leave the device value alone.");
    });
}
pub fn wheel(ui: &mut Ui, h: &mut HardwareSettings, s: &Snapshot) {
    card().show(ui,|ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new("Wheel mode & SmartShift").strong());
        muted(ui,match (s.wheel_mode,s.smartshift) {
            (Some(m),Some(t))=>format!("Live: {} · SmartShift {}",m.label(),if t==50{"off".into()}else{t.to_string()}),
            _=>"Live wheel settings unavailable".into(),
        });
        let supported=!s.available||s.wheel_mode.is_some();
        if !supported {muted(ui,"This mouse does not expose wheel mode or SmartShift.");}
        let mut managed=h.wheel_mode.is_some();
        if ui.add_enabled(supported||managed,Checkbox::new(&mut managed,"Manage wheel mode")).changed(){h.wheel_mode=managed.then_some(s.wheel_mode.unwrap_or(WheelMode::Ratchet));}
        if let Some(mode)=&mut h.wheel_mode {ui.horizontal_wrapped(|ui| {
            ui.selectable_value(mode,WheelMode::Ratchet,"Ratchet");
            ui.selectable_value(mode,WheelMode::FreeSpin,"Free-spin");
        });}
        let mut managed=h.smartshift.is_some();
        if ui.add_enabled(supported||managed,Checkbox::new(&mut managed,"Manage SmartShift")).changed(){h.smartshift=managed.then_some(s.smartshift.unwrap_or(20));}
        if let Some(t)=&mut h.smartshift {
            ui.spacing_mut().slider_width = (ui.available_width() - 150.0).clamp(90.0,160.0);
            ui.add(Slider::new(t,1..=50).text("Threshold"));
            muted(ui,if *t==50 {"Automatic switching off"}else{"Lower values enter free-spin sooner. 50 turns automatic switching off."});
        }
        muted(ui,"Save applies these choices. The physical mode button can still change the wheel afterward.");
    });
}
pub fn access(ui: &mut Ui, s: &Snapshot) {
    if let Some(e) = &s.error {
        ui.colored_label(ERROR, e);
    }
}

pub fn thumbwheel(ui: &mut Ui, h: &mut HardwareSettings, s: &Snapshot) {
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new("Side wheel").strong());
        muted(
            ui,
            "Native horizontal scrolling by default, or remap it to shortcuts, volume, zoom and more.",
        );
        if let Some(state) = s.thumbwheel {
            muted(
                ui,
                if state.diverted && h.thumbwheel_remap.is_some() {
                    "Live: diverted to Mouse Studio actions"
                } else if state.diverted {
                    "Live: diverted · Save to restore horizontal scrolling"
                } else if state.inverted {
                    "Live: native horizontal scrolling · reversed"
                } else {
                    "Live: native horizontal scrolling · normal"
                },
            );
        } else {
            muted(ui, "Live side-wheel state unavailable");
        }
        let mut remapped = h.thumbwheel_remap.is_some();
        ui.horizontal_wrapped(|ui| {
            if ui.selectable_label(!remapped, "Horizontal scroll").clicked() {
                remapped = false;
            }
            if ui.selectable_label(remapped, "Remap to actions").clicked() {
                remapped = true;
            }
        });
        if remapped != h.thumbwheel_remap.is_some() {
            h.thumbwheel_remap = remapped.then(volume_preset);
        }
        if let Some(remap) = &mut h.thumbwheel_remap {
            thumb_remap_editor(ui, remap);
        } else {
            let mut inverted = h.thumbwheel_inverted.unwrap_or(false);
            if ui.checkbox(&mut inverted, "Reverse side wheel").changed() {
                h.thumbwheel_inverted = Some(inverted);
            }
            muted(
                ui,
                "Natural scrolling below also affects this wheel. Scroll speed applies to both wheels.",
            );
        }
    });
}

fn action(kind: ActionKind, value: &str) -> Binding {
    Binding {
        kind,
        value: value.into(),
    }
}
fn volume_preset() -> ThumbWheelRemap {
    ThumbWheelRemap {
        left: action(ActionKind::Shortcut, "XF86AudioLowerVolume"),
        right: action(ActionKind::Shortcut, "XF86AudioRaiseVolume"),
        step: 1,
    }
}
/// Ready-made side-wheel assignments (left, right). All use existing action kinds.
pub fn thumb_presets() -> [(&'static str, ThumbWheelRemap); 4] {
    let zoom = ThumbWheelRemap {
        left: action(ActionKind::Shortcut, "CTRL+minus"),
        right: action(ActionKind::Shortcut, "CTRL+equal"),
        step: 1,
    };
    let workspace = ThumbWheelRemap {
        left: action(ActionKind::Command, "hyprctl dispatch workspace e-1"),
        right: action(ActionKind::Command, "hyprctl dispatch workspace e+1"),
        step: 4,
    };
    let tabs = ThumbWheelRemap {
        left: action(ActionKind::Shortcut, "CTRL+SHIFT+TAB"),
        right: action(ActionKind::Shortcut, "CTRL+TAB"),
        step: 4,
    };
    [
        ("Volume", volume_preset()),
        ("Zoom", zoom),
        ("Workspaces", workspace),
        ("Browser tabs", tabs),
    ]
}
fn thumb_remap_editor(ui: &mut Ui, remap: &mut ThumbWheelRemap) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Presets");
        for (name, preset) in thumb_presets() {
            if ui.small_button(name).clicked() {
                *remap = preset;
            }
        }
        if ui.small_button("Swap directions").clicked() {
            std::mem::swap(&mut remap.left, &mut remap.right);
        }
    });
    for (label, binding) in [
        ("Scroll left", &mut remap.left),
        ("Scroll right", &mut remap.right),
    ] {
        ui.push_id(label, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(label);
                ComboBox::from_id_salt("kind")
                    .selected_text(binding.kind.label())
                    .show_ui(ui, |ui| {
                        for kind in [
                            ActionKind::Default,
                            ActionKind::Shortcut,
                            ActionKind::Application,
                            ActionKind::Command,
                            ActionKind::Workspace,
                        ] {
                            ui.selectable_value(&mut binding.kind, kind, kind.label());
                        }
                    });
                if binding.kind != ActionKind::Default {
                    ui.add(TextEdit::singleline(&mut binding.value).desired_width(180.0));
                }
            });
        });
    }
    ui.horizontal_wrapped(|ui| {
        ui.label("Wheel travel per action");
        ui.add(DragValue::new(&mut remap.step).range(1..=200));
    });
    if let Err(e) = remap.validate() {
        ui.colored_label(ERROR, e);
    }
    muted(
        ui,
        "Remapping diverts the side wheel; the Mouse Studio helper runs the actions and puts native scrolling back when it stops or you switch to Horizontal scroll. Direction and travel units are unverified on hardware: use Swap directions if left and right feel reversed.",
    );
}

/// Returns true when "Test haptic" was pressed.
pub fn haptics(ui: &mut Ui, h: &mut HardwareSettings, s: &Snapshot) -> bool {
    let mut test = false;
    card().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new("Haptic feedback").strong());
        let Some(state) = s.haptics else {
            muted(
                ui,
                if s.available {
                    "This mouse did not report haptic feedback (HID++ 0x19B0); no setting is written."
                } else {
                    "Connect the MX Master 4 with hardware access to discover haptic feedback."
                },
            );
            // A saved setting stays visible so it can be turned off again.
            if h.haptics.is_some() && ui.button("Stop managing haptics").clicked() {
                h.haptics = None;
            }
            return;
        };
        muted(
            ui,
            format!(
                "Live: {} · strength {}%",
                if state.enabled { "on" } else { "off" },
                state.level
            ),
        );
        let mut managed = h.haptics.is_some();
        if ui.checkbox(&mut managed, "Manage haptic feedback").changed() {
            h.haptics = managed.then_some(HapticSettings {
                enabled: state.enabled,
                level: if state.discrete_levels {
                    HAPTIC_LEVELS.iter().map(|l| l.0).min_by_key(|l| l.abs_diff(state.level)).unwrap_or(50)
                } else {
                    state.level.clamp(1, 100)
                },
            });
        }
        if let Some(settings) = &mut h.haptics {
            ui.checkbox(&mut settings.enabled, "Haptic feedback on");
            if settings.enabled {
                if state.discrete_levels {
                    ui.horizontal_wrapped(|ui| {
                        for (level, name) in HAPTIC_LEVELS {
                            ui.selectable_value(&mut settings.level, level, name);
                        }
                    });
                } else {
                    ui.spacing_mut().slider_width = (ui.available_width() - 150.0).clamp(90.0, 160.0);
                    ui.add(Slider::new(&mut settings.level, 1..=100).text("Strength"));
                }
            }
        }
        if ui
            .add_enabled(state.can_play(), Button::new("Test haptic"))
            .clicked()
        {
            test = true;
        }
        muted(
            ui,
            "Save applies the choice and verifies it. Test haptic plays one effect using the mouse's current setting; it does not change any setting.",
        );
    });
    test
}
