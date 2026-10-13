//! Checks compositor APIs on a temporary virtual pointer, never on the physical mouse.
use mouse_studio::{
    backend::{CommandRunner, Hyprctl},
    model::{ActionKind, Binding, Settings},
    runtime,
};
fn ipc(verb: &str, value: &str) -> String {
    let session = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").expect("explicit session required");
    Hyprctl.run(&["-i", &session, verb, value]).unwrap()
}
struct Cleanup;
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = ipc(
            "eval",
            "local s=__mouse_studio_validation_v1; if s then s.cancel(); for _,h in pairs(s.binds) do h:set_enabled(false) end; if s.focus_event then s.focus_event:remove() end; s.timer:set_enabled(false) end; if __mouse_studio_validation_macro_v1 then __mouse_studio_validation_macro_v1.cancel() end; __mouse_studio_validation_v1=nil; __mouse_studio_validation_macro_v1=nil",
        );
    }
}
#[test]
#[ignore = "creates a temporary uinput pointer; requires Mouse Studio capture window focused"]
fn compositor_runtime_and_real_horizontal_axis_work_on_an_isolated_virtual_pointer() {
    assert_eq!(std::env::var("MOUSE_TEST_SOFTWARE").unwrap(), "1");
    let session = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap();
    let active: serde_json::Value = serde_json::from_str(
        &Hyprctl
            .run(&["-i", &session, "activewindow", "-j"])
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        active["class"], "mouse-studio-capture",
        "focus the isolated native preview before emitting test input"
    );
    let mut device = mouse_studio::horizontal::virtual_device().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    let mice: serde_json::Value =
        serde_json::from_str(&Hyprctl.run(&["-i", &session, "devices", "-j"]).unwrap()).unwrap();
    let name = mice["mice"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|m| {
            m["name"]
                .as_str()
                .filter(|n| n.contains("mouse-studio-horizontal"))
        })
        .expect("virtual pointer must be recognized by compositor");
    let mut settings = Settings {
        auto_switch: true,
        ..Default::default()
    };
    settings.active_mut().advanced.hold[0] = Binding {
        kind: ActionKind::Macro,
        value: mouse_studio::software::compose_macro("wait 20").unwrap(),
    };
    settings.active_mut().advanced.precision_button = Some(2);
    settings.add_profile(false);
    settings.active_mut().app_classes = vec!["test-editor".into()];
    settings.active_id = 1;
    let script = runtime::setup_script(&settings, None)
        .unwrap()
        .replace("__mouse_studio_v1", "__mouse_studio_validation_v1")
        .replace(
            "__mouse_studio_macro_v1",
            "__mouse_studio_validation_macro_v1",
        )
        .replace("logitech-mx-anywhere-3s", name);
    assert_eq!(ipc("eval", &script).trim(), "ok");
    let _cleanup = Cleanup;
    assert_eq!(
        ipc("eval", "__mouse_studio_validation_v1.press(3)").trim(),
        "ok"
    );
    assert_eq!(
        ipc("repl", "__mouse_studio_validation_v1.precise").trim(),
        "true"
    );
    assert_eq!(ipc("eval","__mouse_studio_validation_v1.release(3); __mouse_studio_validation_v1.focus({class='test-editor'})").trim(),"ok");
    assert_eq!(
        ipc("repl", "__mouse_studio_validation_v1.profile_id").trim(),
        "2"
    );
    assert_eq!(
        ipc(
            "eval",
            "__mouse_studio_validation_v1.focus(nil); __mouse_studio_validation_v1.press(1)"
        )
        .trim(),
        "ok"
    );
    std::thread::sleep(std::time::Duration::from_millis(550));
    assert_eq!(
        ipc("repl", "__mouse_studio_validation_v1.buttons[1].held").trim(),
        "true"
    );
    assert_eq!(ipc("eval","__mouse_studio_validation_v1.release(1); __mouse_studio_validation_macro_v1.start({{key='F35',mods=''}, {delay=20}, {key='F35',mods=''}})").trim(),"ok");
    std::thread::sleep(std::time::Duration::from_millis(120));
    assert_eq!(
        ipc("repl", "__mouse_studio_validation_macro_v1.steps==nil").trim(),
        "true"
    );
    mouse_studio::horizontal::emit(&mut device, 1).unwrap();
    mouse_studio::horizontal::emit(&mut device, -1).unwrap();
    assert!(
        !mouse_studio::horizontal::socket_path().unwrap().exists(),
        "do not run IPC checks over an existing scroll helper"
    );
    let mut server = mouse_studio::horizontal::Server::new().unwrap();
    mouse_studio::horizontal::send("1").unwrap();
    mouse_studio::horizontal::send("-1").unwrap();
    server.pump().unwrap();
}
