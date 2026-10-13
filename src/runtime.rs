//! Escaped data and shared compositor-side input runtime.
use crate::{
    actions::{self, lua_string},
    model::{Binding, Profile, Settings},
    software::{self, MacroStep},
};

pub(crate) fn scope(source: &str, device: crate::device::MouseModel) -> String {
    source
        .replace("__mouse_studio_macro_v1", device.macro_state())
        .replace("__mouse_studio_v1", device.runtime_state())
}
pub(crate) fn macro_support_for(device: crate::device::MouseModel) -> String {
    scope(macro_support(), device)
}
pub fn macro_support() -> &'static str {
    include_str!("macro.lua")
}
pub fn macro_expression(value: &str) -> Result<String, String> {
    let steps = software::parse_macro(value)?
        .iter()
        .map(|step| {
            Ok(match step {
                MacroStep::Delay(ms) => format!("{{delay={ms}}}"),
                MacroStep::Shortcut(value) => {
                    let (mods, key) = actions::shortcut(value)?;
                    format!("{{mods={},key={}}}", lua_string(&mods), lua_string(&key))
                }
            })
        })
        .collect::<Result<Vec<_>, String>>()?
        .join(",");
    Ok(format!(
        "function() __mouse_studio_macro_v1.start({{{steps}}}) end"
    ))
}
pub(crate) fn action_expression_for(
    value: &Binding,
    device: crate::device::MouseModel,
) -> Result<String, String> {
    // Balanced key taps are essential for delayed actions/macros: no releasePending binds.
    if value.kind == crate::model::ActionKind::Shortcut {
        let (mods, key) = actions::shortcut(&value.value)?;
        let args = format!("mods={},key={}", lua_string(&mods), lua_string(&key));
        return Ok(format!(
            "function() hl.dispatch(hl.dsp.send_key_state({{{args},state=\"down\"}})); hl.dispatch(hl.dsp.send_key_state({{{args},state=\"up\"}})) end"
        ));
    }
    let action = actions::dispatcher(value)?.unwrap_or("false".into());
    Ok(
        if matches!(
            value.kind,
            crate::model::ActionKind::Macro
                | crate::model::ActionKind::CycleDpi
                | crate::model::ActionKind::ActionsRing
        ) {
            scope(&action, device)
        } else {
            action
        },
    )
}
fn array(values: &[Binding], device: crate::device::MouseModel) -> Result<String, String> {
    Ok(format!(
        "{{{}}}",
        values
            .iter()
            .map(|b| action_expression_for(b, device))
            .collect::<Result<Vec<_>, _>>()?
            .join(",")
    ))
}
fn index(index: Option<usize>) -> usize {
    index.map_or(0, |i| i + 1)
}
#[cfg(test)]
fn profile(profile: &Profile) -> Result<String, String> {
    profile_for(profile, Default::default(), actions::MOUSE)
}
fn profile_for(
    profile: &Profile,
    device: crate::device::MouseModel,
    input_name: &str,
) -> Result<String, String> {
    let a = &profile.advanced;
    let clicks=profile.bindings[..device.button_count()].iter().enumerate().map(|(i,b)| {
        if i < 3 && b.kind==crate::model::ActionKind::Default && (software::custom(&a.hold[i]) || software::custom(&a.double[i]) || a.gesture_button==Some(i) || a.horizontal_button==Some(i)) {
            let key=["mouse:275","mouse:276","mouse:274"][i];
            Ok(format!("function() hl.dispatch(hl.dsp.send_key_state({{mods=\"\",key=\"{key}\",state=\"down\"}})); hl.dispatch(hl.dsp.send_key_state({{mods=\"\",key=\"{key}\",state=\"up\"}})) end"))
        } else if i == 3 && b.kind == crate::model::ActionKind::Default && profile.button_managed(i) {
            Ok(format!("function() {}.toggle_wheel() end", device.runtime_state()))
        } else { action_expression_for(b,device) }
    }).collect::<Result<Vec<_>,String>>()?.join(",");
    Ok(format!(
        "{{id={},token={},classes={{{}}},pointer={{name={},sensitivity={},natural_scroll={},accel_profile={},scroll_factor={}}},click={},hold={},double={},hold_ms={},double_ms={},precision={},precision_speed={},horizontal={},gesture={},threshold={},gestures={},dpi_presets={}}}",
        profile.id,
        lua_string(&actions::token(profile)),
        profile
            .app_classes
            .iter()
            .map(|s| lua_string(&s.to_ascii_lowercase()))
            .collect::<Vec<_>>()
            .join(","),
        lua_string(input_name),
        profile.sensitivity,
        profile.natural_scroll,
        lua_string(if profile.acceleration {
            "adaptive"
        } else {
            "flat"
        }),
        profile.scroll_speed,
        format_args!("{{{clicks}}}"),
        array(&a.hold[..device.button_count()], device)?,
        array(&a.double[..device.button_count()], device)?,
        a.hold_ms,
        a.double_ms,
        index(a.precision_button),
        a.precision_speed,
        index(a.horizontal_button),
        index(a.gesture_button),
        a.gesture_threshold,
        array(&a.gestures, device)?,
        lua_string(
            &profile
                .hardware
                .dpi_values()
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )
    ))
}
pub fn setup_script(settings: &Settings, helper: Option<&str>) -> Result<String, String> {
    setup_script_for(settings, helper, settings.device.input_names()[0])
}
pub fn setup_script_for(
    settings: &Settings,
    helper: Option<&str>,
    input_name: &str,
) -> Result<String, String> {
    setup_script_for_inputs(settings, helper, &[input_name.into()])
}
pub fn setup_script_for_inputs(
    settings: &Settings,
    helper: Option<&str>,
    input_names: &[String],
) -> Result<String, String> {
    if input_names.is_empty()
        || input_names.iter().any(|n| {
            !settings.device.input_names().contains(&n.as_str())
                && !crate::hardware::receiver::pointer_name(n)
        })
    {
        return Err("Unknown input device name.".into());
    }
    let input_name = &input_names[0];
    let inputs = input_names
        .iter()
        .map(|n| lua_string(n))
        .collect::<Vec<_>>()
        .join(",");

    settings.validate()?;
    if (settings.horizontal_required()
        || settings.hardware_required()
        || settings.buttons_required()
        || settings.ring_required())
        && helper.is_none()
    {
        return Err(
            "Horizontal scrolling, hardware controls and extra buttons require the installed user helper.".into(),
        );
    }
    let profiles = settings
        .profiles
        .iter()
        .map(|p| profile_for(p, settings.device, input_name))
        .collect::<Result<Vec<_>, _>>()?
        .join(",");
    let helper = helper.map(lua_string).unwrap_or("false".into());
    let script = format!(
        "{}\nlocal input_names={{{inputs}}}\nlocal profiles={{{profiles}}}\nlocal fallback={}\nlocal auto={}\nlocal setup_token={}\nlocal scroll_helper={helper}\nlocal device_slug={}\n{}",
        macro_support_for(settings.device),
        settings.active_id,
        settings.auto_switch,
        lua_string(&setup_token(settings)),
        lua_string(settings.device.slug()),
        scope(include_str!("runtime.lua"), settings.device)
            .replace(
                "local button_count=3",
                &format!("local button_count={}", settings.device.button_count())
            )
            .replace("{\"logitech-mx-anywhere-3s\"}", &format!("{{{inputs}}}"))
            .replace(
                " --cycle-dpi ",
                &if settings.device == crate::device::MouseModel::MxAnywhere3s {
                    " --cycle-dpi ".into()
                } else {
                    format!(" --device {} --cycle-dpi ", settings.device.slug())
                }
            )
            .replace(
                " --horizontal-scroll ",
                &if settings.device == crate::device::MouseModel::MxAnywhere3s {
                    " --horizontal-scroll ".into()
                } else {
                    format!(" --device {} --horizontal-scroll ", settings.device.slug())
                }
            )
            .replace(
                " --toggle-wheel",
                &format!(" --device {} --toggle-wheel", settings.device.slug())
            )
    );
    if script.len() > 100_000 {
        return Err("Profiles exceed the compositor command size limit. Reduce long actions or unused profiles, then Save again.".into());
    }
    Ok(script)
}
pub fn profile_script(profile: &Profile) -> Result<String, String> {
    let settings = Settings {
        active_id: profile.id,
        profiles: vec![profile.clone()],
        ..Default::default()
    };
    setup_script(&settings, None)
}
pub fn setup_token(settings: &Settings) -> String {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_vec(settings)
        .expect("settings serialization")
        .hash(&mut hash);
    format!("{:016x}", hash.finish())
}
pub fn query(settings: &Settings, address: &str) -> String {
    scope(
        &format!(
            "(function() local s=__mouse_studio_v1; if not s or s.setup_token~={} or s.address~={} then return false end; if not s.profile or s.token~=s.profile.token then return false end; for _,h in pairs(s.binds or {{}}) do if h:is_enabled()==nil then return false end end; return s.timer and s.timer:is_enabled()~=nil end)()",
            lua_string(&setup_token(settings)),
            lua_string(address)
        ),
        settings.device,
    )
}

#[cfg(test)]
mod shortcut_dispatch_tests {
    use super::*;
    use crate::backend::{CommandRunner, Hyprctl};

    #[test]
    #[ignore = "checks real Hyprland constructors; intercepts dispatch so no input is sent"]
    fn live_shortcut_dispatchers_construct_without_sending_input() {
        let session = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").expect("explicit test session");
        let mut settings = Settings::default();
        let p = settings.active_mut();
        p.bindings[0] = Binding {
            kind: crate::model::ActionKind::Shortcut,
            value: "CTRL+SHIFT+C".into(),
        };
        p.advanced.hold[1] = p.bindings[0].clone();
        let profile = lua_string(&format!("return {}", profile(p).unwrap()));
        let support = lua_string(macro_support());
        // Keep all generated state inside a private Lua environment. Use the real
        // dispatcher constructors but never call the compositor's hl.dispatch.
        let probe = format!(
            r#"(function()
            local count=0
            local env={{_G={{}},rawget=rawget,math=math,hl={{dsp=hl.dsp}}}}
            env.hl.dispatch=function(d) assert(d~=nil,'Hyprland rejected the dispatcher'); count=count+1 end
            env.hl.timer=function() return {{set_enabled=function() end,is_enabled=function() return true end}} end
            local p=assert(load({profile},'shortcut-check','t',env))()
            p.click[1](); p.click[2]()
            assert(load({support},'macro-check','t',env))()
            local m=env._G.__mouse_studio_macro_v1
            m.start({{{{mods='CTRL',key='c'}}}}); m.tick()
            assert(count==6,'expected three balanced key taps')
            return 'shortcut constructors passed; no input sent'
        end)()"#
        );
        let result = Hyprctl.run(&["-i", &session, "repl", &probe]).unwrap();
        assert!(
            result.contains("shortcut constructors passed; no input sent"),
            "{result}"
        );
    }
}
