use mouse_studio::{
    actions,
    device::MouseModel,
    model::{ActionKind, Binding, Settings},
    recovery, ring,
    ring::{Hit, Outcome, Ring, Session, Slice},
    runtime,
};

fn ring_settings() -> Settings {
    let mut settings = Settings::for_device(MouseModel::MxMaster4);
    let p = settings.active_mut();
    p.bindings[4] = Binding {
        kind: ActionKind::ActionsRing,
        value: String::new(),
    };
    p.advanced.ring = Ring::starter();
    settings
}

#[test]
fn slices_are_clockwise_from_the_top_for_every_slice_count() {
    for n in 2..=8usize {
        for i in 0..n {
            let (dx, dy) = ring::slice_offset(i, n, 80.0);
            assert_eq!(ring::hit(dx, dy, n, 30.0, 130.0), Hit::Slice(i), "{i}/{n}");
        }
    }
    // Eight slices: straight up is slice 0, right is 2, down is 4, left is 6.
    assert_eq!(ring::hit(0.0, -80.0, 8, 30.0, 130.0), Hit::Slice(0));
    assert_eq!(ring::hit(80.0, 0.0, 8, 30.0, 130.0), Hit::Slice(2));
    assert_eq!(ring::hit(0.0, 80.0, 8, 30.0, 130.0), Hit::Slice(4));
    assert_eq!(ring::hit(-80.0, 0.0, 8, 30.0, 130.0), Hit::Slice(6));
    // Boundaries sit halfway between slice centres; slice 0 straddles twelve o'clock.
    assert_eq!(ring::hit(-5.0, -80.0, 8, 30.0, 130.0), Hit::Slice(0));
    assert_eq!(ring::hit(5.0, -80.0, 8, 30.0, 130.0), Hit::Slice(0));
    assert_eq!(ring::hit(80.0, -40.0, 8, 30.0, 130.0), Hit::Slice(1));
}

#[test]
fn centre_and_outside_never_select_a_slice() {
    assert_eq!(ring::hit(0.0, 0.0, 6, 30.0, 130.0), Hit::Center);
    assert_eq!(ring::hit(20.0, -20.0, 6, 30.0, 130.0), Hit::Center);
    assert_eq!(ring::hit(0.0, -131.0, 6, 30.0, 130.0), Hit::Outside);
    assert_eq!(ring::hit(f32::NAN, 1.0, 6, 30.0, 130.0), Hit::Outside);
    assert_eq!(ring::hit(50.0, 0.0, 0, 30.0, 130.0), Hit::Outside);
}

#[test]
fn release_over_a_slice_chooses_only_after_the_button_was_seen_held() {
    let mut s = Session::new(true);
    // A release without ever seeing the button held (a quick tap) never chooses.
    assert_eq!(s.poll(false, Some(Hit::Slice(2))), Outcome::Pending);
    assert_eq!(s.poll(true, Some(Hit::Center)), Outcome::Pending);
    assert_eq!(s.poll(true, Some(Hit::Slice(2))), Outcome::Pending);
    assert_eq!(s.poll(false, Some(Hit::Slice(2))), Outcome::Choose(2));
    // Releasing in the centre or outside switches to click mode and runs nothing.
    for hit in [Some(Hit::Center), Some(Hit::Outside), None] {
        let mut s = Session::new(true);
        s.poll(true, hit);
        assert_eq!(s.poll(false, hit), Outcome::Pending);
        assert_eq!(s.poll(false, Some(Hit::Slice(1))), Outcome::Pending);
    }
    let mut disabled = Session::new(false);
    disabled.poll(true, None);
    assert_eq!(disabled.poll(false, Some(Hit::Slice(1))), Outcome::Pending);
}

#[test]
fn clicks_choose_slices_dismiss_outside_and_ignore_the_centre() {
    let s = Session::new(true);
    assert_eq!(s.click(Some(Hit::Slice(3))), Outcome::Choose(3));
    assert_eq!(s.click(Some(Hit::Outside)), Outcome::Dismiss);
    assert_eq!(s.click(Some(Hit::Center)), Outcome::Pending);
    assert_eq!(s.click(None), Outcome::Pending);
}

#[test]
fn ring_validation_covers_labels_actions_counts_and_geometry() {
    assert!(ring_settings().validate().is_ok());
    let bad = |edit: &dyn Fn(&mut Ring)| {
        let mut settings = ring_settings();
        edit(&mut settings.active_mut().advanced.ring);
        settings.validate().unwrap_err()
    };
    bad(&|r| r.slices.push(r.slices[0].clone())); // nine slices
    bad(&|r| r.slices[0].label = "  ".into());
    bad(&|r| r.slices[0].label = "x".repeat(25));
    bad(&|r| r.slices[0].label = "tab\there".into());
    bad(&|r| r.slices[0].action.kind = ActionKind::Default);
    bad(&|r| r.slices[0].action.kind = ActionKind::ActionsRing);
    bad(&|r| r.slices[0].action.value.clear());
    bad(&|r| r.slices[0].action.value = "CTRL+NOPE".into());
    bad(&|r| r.slices[5].action.value = "100".into()); // workspace
    bad(&|r| r.slices[1].action.value = "a\0b".into());
    bad(&|r| r.radius = f32::NAN);
    bad(&|r| r.radius = 50.0);
    bad(&|r| r.dead_zone = 120.0);
    bad(&|r| r.slices.truncate(1)); // used by a button, needs two slices
    // Unused rings may be empty; CycleDpi needs no value.
    let mut settings = Settings::default();
    assert!(settings.validate().is_ok());
    settings.active_mut().advanced.ring.slices = vec![Slice {
        label: "DPI".into(),
        action: Binding {
            kind: ActionKind::CycleDpi,
            value: String::new(),
        },
    }];
    assert!(settings.validate().is_ok());
}

#[test]
fn old_files_load_with_an_empty_ring_and_rings_round_trip_through_json() {
    let json = serde_json::to_string(&Settings::default()).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
    value["profiles"][0]["advanced"]
        .as_object_mut()
        .unwrap()
        .remove("ring");
    let old: Settings = serde_json::from_value(value).unwrap();
    assert!(old.active().advanced.ring.slices.is_empty());
    // Missing fields inside the ring also fall back to defaults.
    let partial: Ring = serde_json::from_str(r#"{"slices":[]}"#).unwrap();
    assert_eq!(partial, Ring::default());
    let settings = ring_settings();
    let exported = serde_json::to_string_pretty(&settings).unwrap();
    let imported: Settings = serde_json::from_str(&exported).unwrap();
    assert_eq!(imported, settings);
    assert!(imported.validate().is_ok());
}

#[test]
fn previous_setup_restores_the_ring() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mx-master-4.json");
    let first = ring_settings();
    recovery::save_with_history(&first, &path).unwrap();
    let mut second = first.clone();
    second.active_mut().advanced.ring.slices.truncate(3);
    second.active_mut().advanced.ring.radius = 160.0;
    recovery::save_with_history(&second, &path).unwrap();
    assert_eq!(
        recovery::previous_setup(&path).unwrap(),
        Some(first.clone())
    );
    assert_eq!(Settings::load_from(&path).unwrap(), second);
    // The overlay process reads the saved profile's ring.
    let args = ring::Args {
        profile_id: 1,
        model: MouseModel::MxMaster4,
    };
    assert_eq!(ring::load(&path, &args).unwrap().slices.len(), 3);
    assert!(
        ring::load(
            &path,
            &ring::Args {
                profile_id: 9,
                ..args
            }
        )
        .is_err()
    );
    assert!(
        ring::load(
            &path,
            &ring::Args {
                profile_id: 1,
                model: MouseModel::M650L
            }
        )
        .is_err()
    );
}

#[test]
fn reset_clears_the_ring_and_it_changes_the_profile_token() {
    let mut settings = ring_settings();
    let before = actions::token(settings.active());
    mouse_studio::software::reset_profile(settings.active_mut());
    assert!(settings.active().advanced.ring.slices.is_empty());
    assert_ne!(actions::token(settings.active()), before);
    assert!(!settings.active().uses_ring());
}

#[test]
fn command_line_parsing_and_construction() {
    let args = ring::parse_args(&["7".into(), "mx-master-4".into()]).unwrap();
    assert_eq!(args.profile_id, 7);
    assert_eq!(args.model, MouseModel::MxMaster4);
    for bad in [
        vec![],
        vec!["7".into()],
        vec!["x".into(), "mx-master-4".into()],
        vec!["7".into(), "nope".into()],
        vec!["7".into(), "mx-master-4".into(), "extra".into()],
    ] {
        assert!(ring::parse_args(&bad).is_err(), "{bad:?}");
    }
    assert_eq!(
        ring::launch_command("'/h/mouse-studio'", 7, MouseModel::MxMaster4),
        "'/h/mouse-studio' --ring 7 mx-master-4"
    );
    assert_eq!(ring::parse_cursor("123, 456\n"), Some((123.0, 456.0)));
    assert_eq!(ring::parse_cursor(r#"{"x": 5, "y": 6}"#), Some((5.0, 6.0)));
    assert_eq!(ring::parse_cursor("garbage"), None);
    let script = ring::placement_script(292.0, Some((1000.0, 500.0)));
    assert!(script.contains("class:^(mouse-studio-ring)$") && script.contains("x=292,y=292"));
    assert!(ring::placement_script(292.0, None).contains("hl.get_cursor_pos()"));
    assert_eq!(Ring::default().window_size(), 292.0);
}

#[test]
fn ring_button_requires_the_helper_and_the_background_service() {
    let settings = ring_settings();
    assert!(settings.ring_required() && settings.service_required());
    assert!(runtime::setup_script(&settings, None).is_err());
    let script = runtime::setup_script(&settings, Some("'/h/mouse-studio'")).unwrap();
    assert!(script.contains("open_ring") && script.contains("local device_slug=\"mx-master-4\""));
    assert!(!Settings::default().ring_required());
    // Test action must never pretend to open the overlay.
    use mouse_studio::backend::{DeviceBackend, Hyprland};
    let hypr = Hyprland::default().for_device(MouseModel::MxMaster4);
    assert!(hypr.test_action(&settings.active().bindings[4]).is_err());
}

#[test]
fn slice_actions_use_the_existing_dispatchers_and_never_a_ring() {
    for slice in &Ring::starter().slices {
        assert!(
            actions::dispatcher(&slice.action).is_ok(),
            "{}",
            slice.label
        );
    }
    let nested = Binding {
        kind: ActionKind::ActionsRing,
        value: String::new(),
    };
    assert!(
        ring::execute(MouseModel::MxMaster4, &nested).is_err(),
        "a ring never opens itself"
    );
    assert!(ring::execute(MouseModel::MxMaster4, &Binding::default()).is_err());
}

#[test]
fn generated_lua_launches_the_overlay_for_the_pressed_profile() {
    let settings = ring_settings();
    let script = runtime::setup_script(&settings, Some("'/h/mouse-studio'")).unwrap();
    let program = format!(
        r#"
local binds={{}};local fired={{}}
hl={{dsp={{}}}}
function hl.bind(key,callback,opts)
  local h={{enabled=true,callback=callback}}
  function h:set_enabled(b) self.enabled=b end
  function h:is_enabled() return self.enabled end
  table.insert(binds,h);return h
end
function hl.timer(callback,opts) local t={{}}; function t:set_enabled() end; function t:is_enabled() return true end; return t end
function hl.on(name,callback) return {{is_active=function() return true end,remove=function() end}} end
function hl.get_active_window() return nil end
function hl.get_cursor_pos() return {{x=0,y=0}} end
function hl.device(p) end
function hl.dispatch(a) a() end
function hl.dsp.exec_cmd(s) return function() table.insert(fired,s) end end
function hl.dsp.send_key_state(p) return function() end end
function hl.dsp.focus(p) return function() end end
{script}
local s=__mouse_studio_mx_master_4_v1
s.press(2);s.release(2)
assert(#fired==0,'other buttons must not open the ring')
s.press(5);s.release(5)
assert(#fired==1 and fired[1]=="'/h/mouse-studio' --ring 1 mx-master-4",fired[1])
print('ring passed')
"#
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ring.lua");
    std::fs::write(&path, program).unwrap();
    let output = std::process::Command::new("lua")
        .arg(path)
        .output()
        .expect("Install lua for runtime tests");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn only_one_overlay_may_hold_the_lock() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    // XDG_RUNTIME_DIR is process-global; this is the only test in this file touching it.
    let dir = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("XDG_RUNTIME_DIR", dir.path()) };
    let lock = dir.path().join("mouse-studio-ring.lock");

    let first = ring::acquire_lock().expect("first lock");
    assert!(ring::acquire_lock().is_none());
    let mode = std::fs::metadata(&lock).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "the lock file must be owner-only");
    drop(first);
    assert!(ring::acquire_lock().is_some());

    // A symlink at the lock path (as another user could plant in a shared directory) is refused, not followed.
    std::fs::remove_file(&lock).unwrap();
    let victim = dir.path().join("victim");
    std::fs::write(&victim, "keep").unwrap();
    symlink(&victim, &lock).unwrap();
    assert!(
        ring::acquire_lock().is_none(),
        "a symlinked lock path must be refused"
    );
    assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep");
}
