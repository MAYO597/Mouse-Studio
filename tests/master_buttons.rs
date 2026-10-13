use mouse_studio::{
    device::MouseModel,
    model::{ActionKind, Binding, Settings},
    software_ui::{binding, binding_mut},
};

fn command(value: &str) -> Binding {
    Binding {
        kind: ActionKind::Command,
        value: value.into(),
    }
}

#[test]
fn master_extra_triggers_survive_save_load_and_profile_duplication() {
    let mut settings = Settings::for_device(MouseModel::MxMaster4);
    for (button, name) in [(3, "mode"), (4, "gesture"), (5, "panel")] {
        for trigger in 0..3 {
            *binding_mut(settings.active_mut(), button, trigger) =
                command(&format!("{name}-{trigger}"));
        }
    }
    settings.active_mut().advanced.gesture_button = None;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("master.json");
    settings.save_to(&path).unwrap();
    let mut loaded = Settings::load_from(&path).unwrap();
    loaded.add_profile(true);
    for (button, name) in [(3, "mode"), (4, "gesture"), (5, "panel")] {
        for trigger in 0..3 {
            assert_eq!(
                binding(loaded.active(), button, trigger).value,
                format!("{name}-{trigger}")
            );
        }
    }
}

#[test]
fn legacy_three_button_files_load_without_losing_existing_assignments() {
    let mut old = serde_json::to_value(Settings::for_device(MouseModel::MxMaster4)).unwrap();
    for field in ["bindings", "hold", "double"] {
        let array = if field == "bindings" {
            &mut old["profiles"][0][field]
        } else {
            &mut old["profiles"][0]["advanced"][field]
        };
        array.as_array_mut().unwrap().truncate(3);
    }
    old["profiles"][0]["bindings"][0] = serde_json::to_value(command("existing-back")).unwrap();
    let loaded: Settings = serde_json::from_value(old).unwrap();
    assert_eq!(binding(loaded.active(), 0, 0).value, "existing-back");
    assert_eq!(binding(loaded.active(), 5, 0).kind, ActionKind::Default);
}

#[test]
fn unavailable_controls_are_rejected_for_other_mice() {
    for model in [MouseModel::MxAnywhere3s, MouseModel::M650L] {
        let mut s = Settings::for_device(model);
        *binding_mut(s.active_mut(), 5, 0) = command("panel");
        assert!(s.validate().is_err());
    }
}

#[test]
fn master_gestures_can_use_the_dedicated_gesture_button() {
    let mut s = Settings::for_device(MouseModel::MxMaster4);
    s.active_mut().advanced.gesture_button = Some(4);
    s.active_mut().advanced.gestures[3] = command("right");
    assert!(s.validate().is_ok());
}

#[test]
fn extra_buttons_require_a_helper_and_execute_their_own_timed_actions() {
    let mut s = Settings::for_device(MouseModel::MxMaster4);
    *binding_mut(s.active_mut(), 3, 0) = command("mode-click");
    *binding_mut(s.active_mut(), 4, 1) = command("gesture-hold");
    *binding_mut(s.active_mut(), 5, 0) = command("panel-click");
    *binding_mut(s.active_mut(), 5, 2) = command("panel-double");
    s.add_profile(false);
    let advanced = &mut s.active_mut().advanced;
    advanced.precision_button = Some(5);
    advanced.gesture_button = Some(4);
    advanced.horizontal_button = Some(3);
    advanced.gestures[3] = command("gesture-right");
    s.active_id = 1;
    assert!(mouse_studio::runtime::setup_script(&s, None).is_err());
    let script = mouse_studio::runtime::setup_script(&s, Some("helper")).unwrap();
    let program = format!(
        r#"
local fired={{}};local binds={{}};local pointer={{}};local cursor={{x=0,y=0}}
hl={{dsp={{}},get_active_window=function() return nil end,get_cursor_pos=function() return cursor end,device=function(p) pointer=p end}}
function hl.bind(key,callback,opts)
  local h={{enabled=true,callback=callback}};function h:set_enabled(b) self.enabled=b end;function h:is_enabled() return self.enabled end
  binds[key]=h;return h
end
function hl.timer(callback,opts) return {{set_enabled=function() end,is_enabled=function() return true end}} end
function hl.on(...) return {{is_active=function() return true end}} end
function hl.dispatch(a) a() end
function hl.dsp.exec_cmd(v) return function() table.insert(fired,v) end end
{script}
local s=__mouse_studio_mx_master_4_v1
s.press(4);s.release(4);assert(fired[1]=='mode-click')
s.press(5);for _=1,25 do s.tick() end;s.release(5);assert(fired[2]=='gesture-hold')
s.press(6);s.release(6);for _=1,5 do s.tick() end
s.press(6);s.release(6);assert(fired[3]=='panel-double')
for _=1,20 do s.tick() end;assert(#fired==3,'double click also fired single click')
s.press(6);s.release(6);assert(not s.pressed[6] and s.flash[6]>0,'extra live state missing')
for _=1,15 do s.tick() end;assert(fired[4]=='panel-click')
s.press(7);s.release(7);assert(#fired==4,'primary click ran a configurable action')
s.press(5);s.cancel();for _=1,30 do s.tick() end;assert(#fired==4,'cancel left pending hold')
s.activate(s.profiles[2])
s.press(6);assert(pointer.sensitivity==-0.65 and pointer.accel_profile=='flat')
s.release(6);assert(pointer.sensitivity==0 and pointer.accel_profile=='adaptive')
s.press(5);cursor.x=100;s.release(5);assert(fired[5]=='gesture-right')
s.press(4);s.wheel(1);s.release(4);assert(fired[6]=='helper --device mx-master-4 --horizontal-scroll 1')
assert(#fired==6,'horizontal scroll also ran default click')
s.press(4);s.release(4);assert(fired[7]=='helper --device mx-master-4 --toggle-wheel','short default mode click must still toggle the wheel')
print('extra buttons passed')
"#
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("extra.lua");
    std::fs::write(&path, program).unwrap();
    let out = std::process::Command::new("lua")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn extra_event_bridge_executes_only_the_profile_that_captured_the_event() {
    use mouse_studio::backend::{CommandRunner, Hyprland};
    use std::sync::Mutex;
    struct LuaRunner {
        prefix: String,
        output: Mutex<String>,
    }
    impl CommandRunner for LuaRunner {
        fn run(&self, args: &[&str]) -> Result<String, String> {
            assert_eq!(&args[..3], ["-i", "chosen-session", "eval"]);
            let program = format!(
                "{}\n{}\nprint(table.concat(fired,'|'))",
                self.prefix, args[3]
            );
            let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
            let path = dir.path().join("bridge.lua");
            std::fs::write(&path, program).map_err(|e| e.to_string())?;
            let out = std::process::Command::new("lua")
                .arg(path)
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).into());
            }
            *self.output.lock().unwrap() = String::from_utf8_lossy(&out.stdout).trim().into();
            Ok("ok".into())
        }
    }
    let mut settings = Settings::for_device(MouseModel::MxMaster4);
    settings.active_mut().bindings[5] = command("panel-action");
    let runtime = mouse_studio::runtime::setup_script(&settings, Some("helper")).unwrap();
    let support = r#"
local fired={}
hl={dsp={},device=function() end,get_active_window=function() return nil end,get_cursor_pos=function() return {x=0,y=0} end}
function hl.bind(...) return {set_enabled=function() end,is_enabled=function() return true end} end
function hl.timer(...) return {set_enabled=function() end,is_enabled=function() return true end} end
function hl.on(...) return {is_active=function() return true end} end
function hl.dispatch(a) a() end
function hl.dsp.exec_cmd(v) return function() table.insert(fired,v) end end
"#;
    for (steering, expected) in [
        ("", "panel-action"),
        ("s.setup_token='new-save'", ""),
        ("s.profile_id=2", ""),
        ("s.token='edited-profile'", ""),
        ("s.address=nil", ""),
    ] {
        let backend = Hyprland::with_runner(LuaRunner {
            prefix: format!("{support}\n{runtime}\nlocal s=__mouse_studio_mx_master_4_v1;s.address='receiver';{steering}"),
            output: Mutex::new(String::new()),
        },Some("chosen-session".into()),None).for_device(MouseModel::MxMaster4);
        backend
            .button_events(
                "chosen-session",
                &settings,
                settings.active(),
                &[(5, true), (5, false)],
            )
            .unwrap();
        assert_eq!(
            *backend.runner.output.lock().unwrap(),
            expected,
            "{steering}"
        );
        assert!(
            backend
                .button_events("chosen-session", &settings, settings.active(), &[(6, true)])
                .is_err()
        );
    }
}
