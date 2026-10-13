use mouse_studio::{
    hardware::{self, Client, HardwareSettings, Transport, WheelMode},
    model::Settings,
};
use std::collections::VecDeque;

struct Wire(VecDeque<(u8, u8, Vec<u8>, Vec<u8>)>);
impl Transport for Wire {
    fn request(&mut self, feature: u8, function: u8, params: &[u8]) -> Result<Vec<u8>, String> {
        let (f, n, p, r) = self.0.pop_front().expect("unexpected device request");
        assert_eq!((feature, function, params), (f, n, p.as_slice()));
        Ok(r)
    }
}
fn wire(steps: Vec<(u8, u8, Vec<u8>, Vec<u8>)>) -> Client<Wire> {
    Client::new(Wire(steps.into()))
}

#[test]
fn dpi_ranges_expand_and_bad_ranges_are_rejected() {
    assert_eq!(
        hardware::decode_dpi_list(&[0x01, 0x90, 0xe0, 0x64, 0x03, 0x20, 0, 0]).unwrap(),
        vec![400, 500, 600, 700, 800]
    );
    assert!(hardware::decode_dpi_list(&[0xe0, 0x64, 0x03, 0x20]).is_err());
    assert!(hardware::decode_dpi_list(&[0x01, 0x90, 0xe0, 0x00, 0x03, 0x20]).is_err());
    assert!(hardware::decode_dpi_list(&[0x01]).is_err());
    // A long HID++ reply has 16 parameter bytes: one sensor byte and 15 list bytes.
    let padded = hardware::decode_dpi_list(&[
        0x01, 0x90, 0xe0, 0x64, 0x1f, 0x40, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ])
    .unwrap();
    assert_eq!(padded.len(), 77);
    assert_eq!(padded.first(), Some(&400));
    assert_eq!(padded.last(), Some(&8000));
}

#[test]
fn reply_matching_ignores_notifications_and_reports_device_errors() {
    assert_eq!(
        hardware::match_reply(&[0x11, 0, 14, 0x28, 0, 0x06, 0x40], 0xff, 14, 0x28).unwrap(),
        Some(vec![0, 6, 0x40])
    );
    assert_eq!(
        hardware::match_reply(&[0x11, 0xff, 14, 0x20, 9], 0xff, 14, 0x28).unwrap(),
        None
    );
    assert_eq!(
        hardware::match_reply(&[0x11, 2, 14, 0x28, 9], 0xff, 14, 0x28).unwrap(),
        None
    );
    assert!(hardware::match_reply(&[0x11, 0xff, 0xff, 14, 0x28, 2], 0xff, 14, 0x28).is_err());
    assert!(hardware::match_reply(&[0x10, 0xff, 0x8f, 14, 0x28, 1, 0], 0xff, 14, 0x28).is_err());
}

#[test]
fn battery_is_from_the_mouse_and_disappearance_clears_it() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("ACAD")).unwrap();
    for (name, model, capacity, status) in [
        ("BAT1", "Laptop", "99", "Charging"),
        ("hidpp_battery_5", "MX Anywhere 3S", "55", "Discharging"),
    ] {
        let p = dir.path().join(name);
        std::fs::create_dir(&p).unwrap();
        for (f, v) in [
            ("model_name", model),
            ("capacity", capacity),
            ("status", status),
        ] {
            std::fs::write(p.join(f), v).unwrap();
        }
    }
    let b = hardware::read_battery(dir.path()).unwrap();
    assert_eq!(b.percent, Some(55));
    assert_eq!(b.status, "Discharging");
    std::fs::write(dir.path().join("hidpp_battery_5/status"), "Charging").unwrap();
    assert_eq!(
        hardware::read_battery(dir.path()).unwrap().status,
        "Charging"
    );
    std::fs::write(dir.path().join("hidpp_battery_5/capacity"), "101").unwrap();
    assert_eq!(hardware::read_battery(dir.path()).unwrap().percent, None);
    std::fs::remove_dir_all(dir.path().join("hidpp_battery_5")).unwrap();
    assert!(hardware::read_battery(dir.path()).is_none());
}

#[test]
fn old_profiles_leave_hardware_unmanaged_and_presets_validate() {
    let mut json = serde_json::to_value(Settings::default()).unwrap();
    json["profiles"][0]
        .as_object_mut()
        .unwrap()
        .remove("hardware");
    let mut settings: Settings = serde_json::from_value(json).unwrap();
    assert!(!settings.active().hardware.managed());
    settings.active_mut().hardware.dpi = Some(1600);
    settings.validate().unwrap();
    settings.active_mut().hardware.presets[0].dpi = 0;
    assert!(settings.validate().is_err());
}

#[test]
fn cycle_uses_preset_order_and_wraps() {
    assert_eq!(hardware::next_dpi(&[800, 1600, 3200], 1600).unwrap(), 3200);
    assert_eq!(hardware::next_dpi(&[800, 1600, 3200], 3200).unwrap(), 800);
    assert_eq!(hardware::next_dpi(&[800, 1600, 3200], 1200).unwrap(), 800);
    assert!(hardware::next_dpi(&[], 800).is_err());
}

#[test]
fn dpi_apply_discovers_support_writes_and_reads_back() {
    let mut c = wire(vec![
        (0, 0, vec![0x22, 1], vec![14, 0, 2]),
        (
            14,
            0x10,
            vec![0, 0, 0],
            vec![0, 0x01, 0x90, 0xe0, 0x64, 0x1f, 0x40, 0, 0],
        ),
        (14, 0x20, vec![0], vec![0, 0x03, 0x20, 0x03, 0x20]),
        (14, 0x30, vec![0, 0x06, 0x40], vec![0]),
        (14, 0x20, vec![0], vec![0, 0x06, 0x40, 0x03, 0x20]),
    ]);
    c.apply(&HardwareSettings {
        dpi: Some(1600),
        ..Default::default()
    })
    .unwrap();
}

#[test]
fn unsupported_dpi_is_rejected_before_any_write() {
    let mut c = wire(vec![
        (0, 0, vec![0x22, 1], vec![14, 0, 2]),
        (
            14,
            0x10,
            vec![0, 0, 0],
            vec![0, 0x03, 0x20, 0x06, 0x40, 0, 0],
        ),
        (14, 0x20, vec![0], vec![0, 0x03, 0x20, 0x03, 0x20]),
    ]);
    assert!(
        c.apply(&HardwareSettings {
            dpi: Some(900),
            ..Default::default()
        })
        .unwrap_err()
        .contains("900")
    );
}

#[test]
fn failed_dpi_readback_restores_the_previous_value() {
    let mut c = wire(vec![
        (0, 0, vec![0x22, 1], vec![14, 0, 2]),
        (
            14,
            0x10,
            vec![0, 0, 0],
            vec![0, 0x03, 0x20, 0x06, 0x40, 0, 0],
        ),
        (14, 0x20, vec![0], vec![0, 0x03, 0x20, 0x03, 0x20]),
        (14, 0x30, vec![0, 0x06, 0x40], vec![0]),
        (14, 0x20, vec![0], vec![0, 0x03, 0x20, 0x03, 0x20]),
        (14, 0x30, vec![0, 0x03, 0x20], vec![0]),
        (14, 0x20, vec![0], vec![0, 0x03, 0x20, 0x03, 0x20]),
    ]);
    assert!(
        c.apply(&HardwareSettings {
            dpi: Some(1600),
            ..Default::default()
        })
        .unwrap_err()
        .contains("restored")
    );
}

#[test]
fn enhanced_smartshift_uses_its_own_functions_and_preserves_torque() {
    let mut c = wire(vec![
        (0, 0, vec![0x21, 0x10], vec![0, 0, 0]),
        (0, 0, vec![0x21, 0x11], vec![20, 0, 0]),
        (20, 0x10, vec![], vec![2, 20, 70]),
        (20, 0x20, vec![1, 255, 0], vec![0]),
        (20, 0x10, vec![], vec![1, 255, 70]),
    ]);
    c.apply(&HardwareSettings {
        wheel_mode: Some(WheelMode::FreeSpin),
        smartshift: Some(50),
        ..Default::default()
    })
    .unwrap();
}

#[test]
fn cycle_action_needs_the_helper_and_validates_without_command_text() {
    use mouse_studio::model::{ActionKind, Binding};
    let mut s = Settings::default();
    s.active_mut().bindings[0] = Binding {
        kind: ActionKind::CycleDpi,
        value: String::new(),
    };
    s.validate().unwrap();
    assert!(s.hardware_required());
    assert!(mouse_studio::runtime::setup_script(&s, None).is_err());
    let program =
        mouse_studio::runtime::setup_script(&s, Some("'/mouse studio/bin/mouse-studio'")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cycle.lua");
    // Exercise the generated action with a real Lua interpreter at the dispatch boundary.
    let script = format!(
        r#"
hl={{dsp={{}}}}; local result
function hl.dsp.exec_cmd(c) return c end
function hl.dispatch(c) if type(c)=="function" then c() else result=c end end
function hl.device(p) end
function hl.get_active_window() return nil end
function hl.get_cursor_pos() return {{x=0,y=0}} end
function hl.bind(k,c,o) return {{set_enabled=function() end,is_enabled=function() return true end}} end
function hl.timer(c,o) return {{set_enabled=function() end,is_enabled=function() return true end}} end
function hl.on(n,c) return {{is_active=function() return true end}} end
{program}
__mouse_studio_v1.press(1)
assert(result=="'/mouse studio/bin/mouse-studio' --cycle-dpi 800,1600,3200",tostring(result))
"#
    );
    std::fs::write(&path, script).unwrap();
    let o = std::process::Command::new("lua")
        .arg(path)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn hardware_follows_focus_and_reconnect_without_overwriting_cycle_changes() {
    let mut s = Settings::default();
    s.active_mut().hardware.dpi = Some(800);
    s.add_profile(false);
    s.active_mut().hardware.dpi = Some(1600);
    s.active_id = 1;
    let mut follower = hardware::desktop::Follower::default();
    let mut applied = Vec::new();
    let mut record = |h: &HardwareSettings| {
        applied.push(h.dpi);
        Ok(())
    };
    assert!(
        follower
            .step(&s, Some(1), Some("mouse1"), &mut record)
            .unwrap()
    );
    assert!(
        !follower
            .step(&s, Some(1), Some("mouse1"), &mut record)
            .unwrap()
    );
    assert!(
        follower
            .step(&s, Some(2), Some("mouse1"), &mut record)
            .unwrap()
    );
    assert!(follower.step(&s, Some(2), None, &mut record).is_err());
    assert!(
        follower
            .step(&s, Some(2), Some("mouse2"), &mut record)
            .unwrap()
    );
    assert!(
        follower
            .step(&s, Some(99), Some("mouse2"), &mut record)
            .is_err()
    );
    assert_eq!(applied, vec![Some(800), Some(1600), Some(1600)]);
    s.active_id = 2;
    s.active_mut().hardware.dpi = Some(3200);
    assert!(
        follower
            .step(&s, Some(2), Some("mouse2"), |_| Err("disconnected".into()))
            .is_err()
    );
    assert!(
        follower
            .step(&s, Some(2), Some("mouse2"), |h| {
                assert_eq!(h.dpi, Some(3200));
                Ok(())
            })
            .unwrap()
    );
}

#[test]
fn unified_battery_reports_receiver_charge_and_rejects_invalid_percent() {
    let mut c = wire(vec![
        (0, 0, vec![0x10, 4], vec![9]),
        (9, 0x10, vec![], vec![90, 8, 0, 0]),
    ]);
    let b = c.battery().unwrap().unwrap();
    assert_eq!(b.percent, Some(90));
    assert_eq!(b.status, "Discharging");
    let mut c = wire(vec![
        (0, 0, vec![0x10, 4], vec![9]),
        (9, 0x10, vec![], vec![101, 8, 0, 0]),
    ]);
    assert!(c.battery().is_err());
    let mut c = wire(vec![(0, 0, vec![0x10, 4], vec![0])]);
    assert!(c.battery().unwrap().is_none());
}
