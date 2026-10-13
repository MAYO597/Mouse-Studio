use mouse_studio::{
    backend::{CommandRunner, Hyprland},
    device::MouseModel,
    hardware::{
        Client, HardwareSettings, ThumbWheelRemap, Transport,
        buttons::{ThumbSteps, ThumbWheel},
        desktop::Follower,
    },
    model::{ActionKind, Binding, Settings},
};
use std::{collections::VecDeque, sync::Mutex};

type Request = (u8, u8, Vec<u8>, Result<Vec<u8>, String>);
struct Wire(VecDeque<Request>);
impl Transport for Wire {
    fn request(&mut self, feature: u8, function: u8, params: &[u8]) -> Result<Vec<u8>, String> {
        let (f, n, p, r) = self.0.pop_front().expect("Unexpected hardware request");
        assert_eq!((feature, function, params), (f, n, p.as_slice()));
        r
    }
}
fn wire(list: Vec<Request>) -> Wire {
    Wire(list.into())
}
fn found() -> Request {
    (0, 0, vec![0x21, 0x50], Ok(vec![18, 0, 0]))
}
fn status(a: u8, b: u8) -> Request {
    (18, 0x10, vec![], Ok(vec![a, b]))
}
fn shortcut(v: &str) -> Binding {
    Binding {
        kind: ActionKind::Shortcut,
        value: v.into(),
    }
}
fn remap() -> ThumbWheelRemap {
    ThumbWheelRemap {
        left: shortcut("XF86AudioLowerVolume"),
        right: shortcut("XF86AudioRaiseVolume"),
        step: 2,
    }
}

#[test]
fn diversion_changes_only_the_divert_bit_and_verifies_readback() {
    let mut w = wire(vec![
        found(),
        status(0x80, 1),
        (18, 0x20, vec![0x81, 1], Ok(vec![])),
        status(0x81, 1),
        // Release
        status(0x81, 1),
        (18, 0x20, vec![0x80, 1], Ok(vec![])),
        status(0x80, 1),
    ]);
    let mut t = ThumbWheel::discover(&mut w).unwrap();
    t.sync(&mut w, true).unwrap();
    assert!(t.active());
    t.sync(&mut w, true).unwrap();
    t.sync(&mut w, false).unwrap();
    assert!(!t.active());
    assert!(w.0.is_empty());
}

#[test]
fn a_mouse_without_a_side_wheel_is_never_written() {
    let mut w = wire(vec![(0, 0, vec![0x21, 0x50], Ok(vec![0, 0, 0]))]);
    assert!(ThumbWheel::discover(&mut w).is_err());
}

#[test]
fn failed_diversion_is_undone_so_the_wheel_is_never_left_without_a_consumer() {
    let mut w = wire(vec![
        found(),
        status(0, 0),
        (18, 0x20, vec![1, 0], Ok(vec![])),
        status(0, 0), // readback shows diversion did not stick
        // cleanup
        status(0, 0),
    ]);
    let mut t = ThumbWheel::discover(&mut w).unwrap();
    assert!(t.sync(&mut w, true).is_err());
    assert!(!t.active());
    // A write that did land but could not be read back is cleared, or retried later.
    let mut w = wire(vec![
        found(),
        status(0, 0),
        (18, 0x20, vec![1, 0], Ok(vec![])),
        status(1, 0),
        status(1, 0),
        (18, 0x20, vec![0, 0], Err("timeout".into())),
    ]);
    let mut t = ThumbWheel::discover(&mut w).unwrap();
    // readback matched here, so it is active; now fail the release.
    t.sync(&mut w, true).unwrap();
    assert!(t.sync(&mut w, false).is_err());
    assert!(t.active(), "ownership is kept until cleanup succeeds");
    let mut w = wire(vec![
        status(1, 0),
        (18, 0x20, vec![0, 0], Ok(vec![])),
        status(0, 0),
    ]);
    t.sync(&mut w, false).unwrap();
    assert!(!t.active());
}

#[test]
fn periodic_verification_redivert_after_wake_or_reconnect() {
    let mut w = wire(vec![
        found(),
        status(0, 0),
        (18, 0x20, vec![1, 0], Ok(vec![])),
        status(1, 0),
        // five-second check: device forgot diversion
        status(0, 0),
        status(0, 0),
        (18, 0x20, vec![1, 0], Ok(vec![])),
        status(1, 0),
        // five-second check: still diverted, nothing is written
        status(1, 0),
    ]);
    let mut t = ThumbWheel::discover(&mut w).unwrap();
    t.sync(&mut w, true).unwrap();
    let reset = t.verify(&mut w, true).unwrap();
    assert!(
        reset,
        "a reset must be reported so partial rotation is dropped"
    );
    assert!(!t.verify(&mut w, true).unwrap());
    assert!(w.0.is_empty());
}

fn report(address: u8, feature: u8, rotation: i16, status: u8) -> Vec<u8> {
    let r = rotation.to_be_bytes();
    let mut v = vec![0x11, address, feature, 0, r[0], r[1], 0, 0, status, 0];
    v.resize(20, 0);
    v
}

#[test]
fn rotation_accumulates_into_steps_and_ignores_foreign_reports() {
    let mut s = ThumbSteps::default();
    assert!(s.update(&report(2, 18, 1, 1), 2, 18, true, 3).is_empty());
    assert!(s.update(&report(2, 18, 1, 2), 2, 18, true, 3).is_empty());
    assert_eq!(s.update(&report(2, 18, 1, 2), 2, 18, true, 3), [true]);
    // Large rotation produces several steps and leaves the remainder.
    assert_eq!(s.update(&report(2, 18, 7, 2), 2, 18, true, 3), [true, true]);
    assert_eq!(s.update(&report(2, 18, 2, 2), 2, 18, true, 3), [true]);
    // Wrong slot, feature, a command reply and short reports are ignored.
    assert!(s.update(&report(1, 18, 9, 2), 2, 18, true, 3).is_empty());
    assert!(s.update(&report(2, 19, 9, 2), 2, 18, true, 3).is_empty());
    let mut reply = report(2, 18, 9, 2);
    reply[3] = 0x18;
    assert!(s.update(&reply, 2, 18, true, 3).is_empty());
    assert!(
        s.update(&report(2, 18, 9, 2)[..8], 2, 18, true, 3)
            .is_empty()
    );
}

#[test]
fn direction_changes_stops_and_inactive_captures_drop_leftover_travel() {
    let mut s = ThumbSteps::default();
    assert!(s.update(&report(2, 18, 2, 2), 2, 18, true, 3).is_empty());
    // Reversing never inherits the two units of rightward travel.
    assert!(s.update(&report(2, 18, -2, 2), 2, 18, true, 3).is_empty());
    assert_eq!(s.update(&report(2, 18, -1, 2), 2, 18, true, 3), [false]);
    // Stop discards the remainder.
    assert!(s.update(&report(2, 18, 2, 3), 2, 18, true, 3).is_empty());
    assert!(s.update(&report(2, 18, 2, 2), 2, 18, true, 3).is_empty());
    // Not diverted by us: never emit.
    assert!(s.update(&report(2, 18, 30, 2), 2, 18, false, 3).is_empty());
    // A flood is bounded.
    assert_eq!(
        s.update(&report(2, 18, i16::MAX, 2), 2, 18, true, 1).len(),
        16
    );
}

#[test]
fn remap_validation_accepts_shortcuts_and_rejects_unsafe_choices() {
    remap().validate().unwrap();
    let mut one_side = remap();
    one_side.left = Binding::default();
    one_side.validate().unwrap();
    let mut none = remap();
    none.left = Binding::default();
    none.right = Binding::default();
    assert!(none.validate().is_err());
    for kind in [ActionKind::Macro, ActionKind::CycleDpi] {
        let mut bad = remap();
        bad.right = Binding {
            kind,
            value: "CTRL+C".into(),
        };
        assert!(bad.validate().is_err(), "{kind:?}");
    }
    let mut empty = remap();
    empty.right.value = "  ".into();
    assert!(empty.validate().is_err());
    let mut bad_key = remap();
    bad_key.right.value = "not a key".into();
    assert!(bad_key.validate().is_err());
    for step in [0, 201] {
        let mut bad = remap();
        bad.step = step;
        assert!(bad.validate().is_err());
    }
}

#[test]
fn profiles_round_trip_duplicate_and_require_the_helper() {
    let mut s = Settings::for_device(MouseModel::MxMaster4);
    assert!(!s.buttons_required());
    s.active_mut().hardware.thumbwheel_remap = Some(remap());
    assert!(s.buttons_required(), "remapping needs the diversion helper");
    assert!(s.service_required());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("master.json");
    s.save_to(&path).unwrap();
    let mut loaded = Settings::load_from(&path).unwrap();
    assert_eq!(loaded.active().hardware.thumbwheel_remap, Some(remap()));
    loaded.add_profile(true);
    assert_eq!(loaded.active().hardware.thumbwheel_remap, Some(remap()));
    loaded.add_profile(false);
    assert_eq!(
        loaded.active().hardware.thumbwheel_remap,
        None,
        "native by default"
    );
    // Corrupt imported JSON is rejected by validation.
    let mut bad = serde_json::to_value(&s).unwrap();
    bad["profiles"][0]["hardware"]["thumbwheel_remap"]["step"] = 0.into();
    std::fs::write(&path, bad.to_string()).unwrap();
    assert!(Settings::load_from(&path).is_err());
}

#[test]
fn only_the_master_4_accepts_side_wheel_actions() {
    for model in [MouseModel::MxAnywhere3s, MouseModel::M650L] {
        let mut s = Settings::for_device(model);
        s.active_mut().hardware.thumbwheel_remap = Some(remap());
        assert!(s.validate().is_err(), "{model:?}");
        assert!(!s.buttons_required());
    }
}

#[test]
fn applying_a_remapped_profile_never_touches_the_side_wheel() {
    // Empty queue: any request, including the feature lookup, would panic.
    let mut w = Client::new(wire(vec![]));
    let mut desired = HardwareSettings {
        thumbwheel_remap: Some(remap()),
        thumbwheel_inverted: Some(false),
        ..Default::default()
    };
    w.apply(&desired).unwrap();
    desired.thumbwheel_remap = None;
    let steps = vec![
        found(),
        status(1, 0),
        (18, 0x20, vec![0, 0], Ok(vec![])),
        status(0, 0),
    ];
    Client::new(wire(steps)).apply(&desired).unwrap();
}

#[test]
fn restore_skips_native_repair_for_remapped_profiles_but_repairs_native_ones() {
    let mut s = Settings::for_device(MouseModel::MxMaster4);
    s.active_mut().hardware.thumbwheel_remap = Some(remap());
    let mut follower = Follower::default();
    follower
        .step(&s, None, Some("bolt"), |h| {
            assert_eq!(h.thumbwheel_remap, Some(remap()));
            Ok(())
        })
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5100));
    assert!(
        !follower
            .step(&s, None, Some("bolt"), |_| panic!(
                "periodic native repair would undo the diversion"
            ))
            .unwrap()
    );
    // Switching to a native profile clears diversion again.
    s.add_profile(false);
    assert!(
        follower
            .step(&s, None, Some("bolt"), |h| {
                assert_eq!(h.thumbwheel_remap, None);
                assert_eq!(h.thumbwheel_inverted, Some(false));
                Ok(())
            })
            .unwrap()
    );
}

struct Fake(Mutex<Vec<Vec<String>>>);
impl CommandRunner for Fake {
    fn run(&self, args: &[&str]) -> Result<String, String> {
        self.0
            .lock()
            .unwrap()
            .push(args.iter().map(|s| s.to_string()).collect());
        Ok("ok\n".into())
    }
}

#[test]
fn compositor_receives_balanced_shortcut_dispatches_guarded_by_profile_identity() {
    let mut s = Settings::for_device(MouseModel::MxMaster4);
    s.active_mut().hardware.thumbwheel_remap = Some(remap());
    let backend = Hyprland::with_runner(Fake(Mutex::new(vec![])), None, None)
        .for_device(MouseModel::MxMaster4);
    backend
        .thumb_events("session", &s, s.active(), &[true, false, true])
        .unwrap();
    let calls = backend.runner.0.lock().unwrap();
    assert_eq!(&calls[0][..3], &["-i", "session", "eval"]);
    let lua = &calls[0][3];
    assert!(lua.contains("__mouse_studio_mx_master_4_v1"));
    assert!(lua.contains(&format!("s.profile_id=={}", s.active().id)));
    assert!(lua.contains("s.setup_token=="));
    assert_eq!(lua.matches("hl.dispatch(").count(), 3 * 2 + 3);
    assert!(lua.matches("XF86AudioRaiseVolume").count() == 4);
    assert!(lua.contains("XF86AudioLowerVolume"));
}

#[test]
fn unassigned_directions_and_native_profiles_send_nothing() {
    let mut s = Settings::for_device(MouseModel::MxMaster4);
    let backend = Hyprland::with_runner(Fake(Mutex::new(vec![])), None, None)
        .for_device(MouseModel::MxMaster4);
    backend
        .thumb_events("session", &s, s.active(), &[true])
        .unwrap();
    let mut one = remap();
    one.left = Binding::default();
    s.active_mut().hardware.thumbwheel_remap = Some(one);
    backend
        .thumb_events("session", &s, s.active(), &[false])
        .unwrap();
    assert!(backend.runner.0.lock().unwrap().is_empty());
}

#[test]
fn a_flood_of_steps_is_capped_per_compositor_call() {
    let mut s = Settings::for_device(MouseModel::MxMaster4);
    s.active_mut().hardware.thumbwheel_remap = Some(ThumbWheelRemap {
        left: Binding {
            kind: ActionKind::Command,
            value: "echo".into(),
        },
        ..remap()
    });
    let backend = Hyprland::with_runner(Fake(Mutex::new(vec![])), None, None)
        .for_device(MouseModel::MxMaster4);
    backend
        .thumb_events("session", &s, s.active(), &[false; 100])
        .unwrap();
    let lua = backend.runner.0.lock().unwrap()[0][3].clone();
    assert_eq!(
        lua.matches("exec_cmd").count(),
        mouse_studio::backend::THUMB_STEPS_PER_BATCH
    );
}
