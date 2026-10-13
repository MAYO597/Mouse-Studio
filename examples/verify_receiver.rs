//! Opt-in receiver runtime probe: watches clicks without consuming or sending input.
use mouse_studio::{
    backend::{CommandRunner, Hyprctl},
    device::MouseModel,
    model::Settings,
    runtime,
};
fn ipc(verb: &str, script: &str) -> String {
    let s = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").expect("Select session");
    Hyprctl.run(&["-i", &s, verb, script]).unwrap()
}
struct Cleanup;
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = ipc(
            "eval",
            "local s=__mouse_studio_bolt_probe; if s then s.cancel(); for _,route in pairs(s.binding_routes or {}) do for _,h in pairs(route) do h:set_enabled(false) end end; if s.focus_event then s.focus_event:remove() end; s.timer:set_enabled(false) end; __mouse_studio_bolt_probe_owner=nil; __mouse_studio_bolt_probe=nil; __mouse_studio_bolt_macro_probe=nil",
        );
    }
}
fn main() {
    mouse_studio::hardware::receiver::verify_pointer(MouseModel::MxMaster4).unwrap();
    let names = vec![
        "logitech-usb-receiver-mouse".into(),
        "logitech-usb-receiver-1".into(),
    ];
    let script = runtime::setup_script_for_inputs(
        &Settings::for_device(MouseModel::MxMaster4),
        None,
        &names,
    )
    .unwrap()
    .replace(
        "__mouse_studio_receiver_owner",
        "__mouse_studio_bolt_probe_owner",
    )
    .replace("__mouse_studio_mx_master_4_v1", "__mouse_studio_bolt_probe")
    .replace(
        "__mouse_studio_mx_master_4_macro_v1",
        "__mouse_studio_bolt_macro_probe",
    );
    // Keep pointer options unchanged. Real Hyprland bindings, timers and events are used.
    let wrapped = format!(
        "local hl=setmetatable({{device=function(p) end}},{{__index=hl}})\n{script}\nlocal s=__mouse_studio_bolt_probe; s.counts={{0,0,0,0,0,0,0,0}}; local press=s.press; s.press=function(i) s.counts[i]=s.counts[i]+1; press(i) end"
    );
    assert_eq!(ipc("eval", &wrapped).trim(), "ok");
    let _cleanup = Cleanup;
    assert_eq!(ipc("repl","(function() local s=__mouse_studio_bolt_probe; for _,h in pairs(s.binds) do if not h.device_inclusive or #h.devices~=2 or not ((h.devices[1]=='logitech-usb-receiver-mouse' and h.devices[2]=='logitech-usb-receiver-1') or (h.devices[2]=='logitech-usb-receiver-mouse' and h.devices[1]=='logitech-usb-receiver-1')) then return false end end; return true end)()").trim(),"true");
    println!("Real receiver button filters installed; monitoring 60 seconds.");
    for _ in 0..60 {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    println!(
        "Physical press counts [Back, Forward, Middle, Left, Right]: {}",
        ipc("repl", "(function() local c=__mouse_studio_bolt_probe.counts; return table.concat({c[1],c[2],c[3],c[7],c[8]},',') end)()").trim()
    );
    println!("Runtime filters verified; passive bindings cleaned up on exit.");
}
