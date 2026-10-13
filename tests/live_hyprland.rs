//! Opt-in integration test. Never run by `cargo test` alone.
//! Supply known effective restoration values: Hyprland exposes scrollFactor,
//! but not effective per-device sensitivity, natural_scroll or accel_profile.
use mouse_studio::backend::{CommandRunner, DeviceBackend, Hyprctl, Hyprland, PointerSettings};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

fn read_env<T: std::str::FromStr>(name: &str) -> T
where
    T::Err: std::fmt::Debug,
{
    std::env::var(name)
        .unwrap_or_else(|_| panic!("provide {name} for restoration"))
        .parse()
        .unwrap()
}

#[test]
#[ignore = "temporarily changes the connected mouse; requires explicit restoration values"]
fn connected_mouse_accepts_four_controls_without_changing_other_devices_or_global_settings() {
    let restore = PointerSettings {
        sensitivity: read_env("MOUSE_TEST_RESTORE_SENSITIVITY"),
        natural_scroll: read_env("MOUSE_TEST_RESTORE_NATURAL_SCROLL"),
        acceleration: read_env("MOUSE_TEST_RESTORE_ACCELERATION"),
        scroll_speed: read_env("MOUSE_TEST_RESTORE_SCROLL_SPEED"),
    };
    assert!(restore.sensitivity.is_finite() && (-1.0..=1.0).contains(&restore.sensitivity));
    assert!(restore.scroll_speed.is_finite() && (0.1..=5.0).contains(&restore.scroll_speed));
    let instance =
        std::env::var("HYPRLAND_INSTANCE_SIGNATURE").expect("select the test session explicitly");
    let runner = Hyprctl;
    let globals = || {
        [
            "input:sensitivity",
            "input:natural_scroll",
            "input:accel_profile",
            "input:scroll_factor",
            "input:force_no_accel",
        ]
        .map(|option| {
            runner
                .run(&["-i", &instance, "getoption", option, "-j"])
                .unwrap()
        })
    };
    let mice = || -> BTreeMap<String, Value> {
        let devices: Value =
            serde_json::from_str(&runner.run(&["-i", &instance, "devices", "-j"]).unwrap())
                .unwrap();
        devices["mice"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| (m["name"].as_str().unwrap().to_string(), m.clone()))
            .collect()
    };
    let errors = || runner.run(&["-i", &instance, "configerrors"]).unwrap();
    let before_globals = globals();
    let before_errors = errors();
    let model = std::env::var("MOUSE_TEST_DEVICE")
        .ok()
        .map(|s| mouse_studio::device::MouseModel::parse(&s).unwrap())
        .unwrap_or_default();
    let names: Vec<String> = std::env::var("MOUSE_TEST_INPUT_NAMES")
        .map(|s| s.split(',').map(str::to_string).collect())
        .unwrap_or_else(|_| vec![model.input_names()[0].into()]);
    let mut other_mice = mice();
    let originals: Vec<_> = names
        .iter()
        .map(|n| other_mice.remove(n).expect("mouse connected"))
        .collect();
    let backend = Hyprland::default().for_device(model);
    struct Restore<'a>(&'a dyn DeviceBackend, PointerSettings);
    impl Drop for Restore<'_> {
        fn drop(&mut self) {
            if let Err(e) = self.0.apply(self.1) {
                eprintln!("RESTORE FAILED: {e}");
            }
        }
    }
    let guard = Restore(&backend, restore);
    let verify_scroll = |expected: f32| {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let mut observed = mice();
            let targets: Vec<_> = names
                .iter()
                .map(|n| observed.remove(n).expect("mouse disconnected during test"))
                .collect();
            assert_eq!(observed, other_mice, "unrelated pointer devices changed");
            let actual = targets[0]["scrollFactor"].as_f64().unwrap();
            assert!(
                targets
                    .iter()
                    .all(|m| (m["scrollFactor"].as_f64().unwrap() - actual).abs() < 0.001)
            );
            if (actual - f64::from(expected)).abs() < 0.001 {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "scroll multiplier readback {actual} != {expected}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    for settings in [
        PointerSettings {
            sensitivity: -0.35,
            natural_scroll: true,
            acceleration: false,
            scroll_speed: 0.6,
        },
        PointerSettings {
            sensitivity: 0.4,
            natural_scroll: false,
            acceleration: true,
            scroll_speed: 1.8,
        },
    ] {
        backend.apply(settings).unwrap();
        verify_scroll(settings.scroll_speed);
        assert_eq!(globals(), before_globals);
        assert_eq!(errors(), before_errors);
    }
    backend
        .apply(restore)
        .expect("restore supplied effective values");
    verify_scroll(restore.scroll_speed);
    assert_eq!(globals(), before_globals);
    assert_eq!(errors(), before_errors);
    // Hyprland reports -1 for inheritance but does not accept it in hl.device.
    // In that case restore the explicitly supplied effective multiplier.
    for original in originals {
        let script = format!(
            "hl.device({{name={},scroll_factor={}}})",
            mouse_studio::actions::lua_string(original["name"].as_str().unwrap()),
            original["scrollFactor"]
                .as_f64()
                .filter(|v| *v >= 0.0)
                .unwrap_or(f64::from(restore.scroll_speed))
        );
        assert_eq!(
            runner
                .run(&["-i", &instance, "eval", &script])
                .unwrap()
                .trim(),
            "ok"
        );
    }
    println!(
        "Both receiver pointers accepted all four controls; scroll readback and unrelated/global settings verified; original effective settings restored."
    );
    std::mem::forget(guard); // Restored successfully; no duplicate compositor write.
}
