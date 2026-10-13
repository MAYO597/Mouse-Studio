use mouse_studio::{
    model::{ActionKind, Binding, Settings},
    runtime,
};
fn command(value: &str) -> Binding {
    Binding {
        kind: ActionKind::Command,
        value: value.into(),
    }
}
#[test]
fn lua_runtime_handles_timing_focus_precision_gestures_and_macro_cancellation() {
    let mut settings = Settings {
        auto_switch: true,
        ..Default::default()
    };
    let p = settings.active_mut();
    p.bindings[0] = command("click");
    p.advanced.hold[0] = command("hold");
    p.bindings[1] = command("single");
    p.advanced.double[1] = command("double");
    p.advanced.precision_button = Some(2);
    settings.add_profile(false);
    let p = settings.active_mut();
    p.app_classes = vec!["Code".into()];
    p.sensitivity = 0.4;
    p.advanced.gesture_button = Some(0);
    p.advanced.gestures[3] = command("right");
    p.bindings[1] = Binding {
        kind: ActionKind::Shortcut,
        value: "CTRL+SHIFT+C".into(),
    };
    p.advanced.hold[2] = command("hold-middle");
    settings.active_id = 1;
    let script = runtime::setup_script(&settings, None).unwrap();
    let others = [
        mouse_studio::device::MouseModel::M650L,
        mouse_studio::device::MouseModel::MxMaster4,
    ]
    .into_iter()
    .map(|model| {
        let mut settings = Settings::for_device(model);
        // A namespace-like command is user data, not a template to rewrite.
        settings.active_mut().bindings[0] = command("echo __mouse_studio_v1");
        format!(
            "do\n{}\nend",
            runtime::setup_script(&settings, None).unwrap()
        )
    })
    .collect::<Vec<_>>()
    .join("\n");
    let receiver = runtime::setup_script_for_inputs(
        &settings,
        None,
        &[
            "logitech-usb-receiver-mouse".into(),
            "logitech-usb-receiver-1".into(),
        ],
    )
    .unwrap();
    let other_receiver = runtime::setup_script_for_inputs(
        &Settings::for_device(mouse_studio::device::MouseModel::MxMaster4),
        None,
        &[
            "logitech-usb-receiver-mouse".into(),
            "logitech-usb-receiver-1".into(),
        ],
    )
    .unwrap();
    let program = format!(
        r#"
local timers={{}};local binds={{}};local events={{}};local fired={{}};local pointer={{}};local pointer_calls={{}};local cursor={{x=0,y=0}}
hl={{dsp={{}}}}
function hl.bind(key,callback,opts)
  assert(opts.device.list[1]=="logitech-mx-anywhere-3s" or opts.device.list[1]=="logitech-signature-m650-l" or opts.device.list[1]=="logitech-mx-master-4" or opts.device.list[1]=="logitech-usb-receiver-mouse")
  local h={{enabled=true,callback=callback,devices=opts.device.list}}
  function h:set_enabled(b) self.enabled=b end
  function h:is_enabled() return self.enabled end
  table.insert(binds,h);return h
end
function hl.timer(callback,opts)
  assert(opts.type=="repeat")
  local t={{enabled=true,callback=callback}}
  function t:set_enabled(b) self.enabled=b end
  function t:is_enabled() return self.enabled end
  table.insert(timers,t);return t
end
function hl.on(name,callback) events[name]=callback;return {{active=true,is_active=function(self) return self.active end,remove=function(self) self.active=false end}} end
function hl.get_active_window() return nil end
function hl.get_cursor_pos() return cursor end
function hl.device(p) pointer=p; pointer_calls[p.name]=p end
function hl.dispatch(a) a() end
function hl.dsp.exec_cmd(s) return function() table.insert(fired,s) end end
function hl.dsp.send_key_state(p)
  assert(p.state=="down" or p.state=="up" or p.state=="repeat", "Hyprland requires a named key state")
  return function() table.insert(fired,p.key..":"..p.state) end
end
function hl.dsp.focus(p) return function() end end
local function step(ms) for _=1,ms/20 do for _,t in ipairs(timers) do if t.enabled then t.callback() end end end end
{script}
local s=__mouse_studio_v1
s.binds.press1.callback();step(500);s.binds.release1.callback()
assert(#fired==1 and fired[1]=="hold")
s.press(2);s.release(2);step(100);assert(#fired==1)
s.press(2);s.release(2);assert(fired[2]=="double");step(400);assert(#fired==2)
s.press(2);s.release(2);step(300);assert(fired[3]=="single")
s.press(3);assert(pointer.sensitivity==-0.65 and s.pressed[3])
s.release(3);assert(pointer.sensitivity==0 and not s.pressed[3])
s.press(3);events['window.active']({{class='CODE'}})
assert(s.profile_id==2 and pointer.sensitivity==0.4 and not s.precise)
s.press(1);cursor.x=100;s.release(1);assert(fired[4]=='right')
__mouse_studio_macro_v1.start({{{{key='c',mods='CTRL'}},{{delay=200}},{{key='v',mods='CTRL'}}}})
step(20);assert(fired[5]=='c:down' and fired[6]=='c:up')
__mouse_studio_macro_v1.cancel();step(400);assert(#fired==6)
s.press(2);s.release(2);assert(fired[7]=='c:down' and fired[8]=='c:up','shortcut must press and release')
s.press(3);s.release(3);assert(fired[9]=='mouse:274:down' and fired[10]=='mouse:274:up','short clicks must preserve the default mouse action')
s.press(1);events['window.active']({{class='Other'}});s.release(1);assert(s.profile_id==1 and #fired==10)
local count=#binds
{script}
assert(#binds==count,'repeated saves must reuse owned handles')
local oldbinds=s.binds
{receiver}
for _,h in pairs(oldbinds) do assert(not h.enabled,'previous transport remains bound') end
assert(s.binds.press1.devices[2]=='logitech-usb-receiver-1')
s.press(3)
assert(pointer_calls['logitech-usb-receiver-mouse'].sensitivity==-0.65)
assert(pointer_calls['logitech-usb-receiver-1'].sensitivity==-0.65)
s.release(3)
local receiverbinds=s.binds
local receivercount=#binds
{script}
assert(s.binds==oldbinds and #binds==receivercount,'returning transport must reuse handles')
for _,h in pairs(receiverbinds) do assert(not h.enabled,'receiver binding remains active') end
{others}
assert(__mouse_studio_v1==s,'installing another model replaced the original runtime')
assert(__mouse_studio_m650_l_v1~=s and __mouse_studio_mx_master_4_v1~=s)
__mouse_studio_m650_l_v1.press(1);__mouse_studio_m650_l_v1.release(1)
assert(fired[#fired]=='echo __mouse_studio_v1','command text must never be rewritten')
__mouse_studio_mx_master_4_v1.press(1);__mouse_studio_mx_master_4_v1.release(1)
assert(fired[#fired]=='echo __mouse_studio_v1')
__mouse_studio_macro_v1.start({{{{key='c',mods='CTRL'}}}})
__mouse_studio_m650_l_macro_v1.start({{{{key='v',mods='CTRL'}}}})
__mouse_studio_m650_l_v1.cancel()
assert(__mouse_studio_macro_v1.steps~=nil and __mouse_studio_m650_l_macro_v1.steps==nil,'cancel must affect only its mouse')
assert(s.profile_id==1)
do
{receiver}
end
local oldreceiver=__mouse_studio_v1
local oldfocus=oldreceiver.focus_event
do
{other_receiver}
end
for _,h in pairs(oldreceiver.binds) do assert(not h.enabled,'previous receiver model remains bound') end
assert(not oldfocus:is_active(),'previous receiver model retains active focus listener')
assert(__mouse_studio_receiver_owner==__mouse_studio_mx_master_4_v1)
print('runtime passed')
"#
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.lua");
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
