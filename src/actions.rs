//! Validated mouse actions. User strings are data inside Lua, never Lua source.
use crate::model::{ActionKind, Binding, Profile};
use std::hash::{Hash, Hasher};

pub const STATE: &str = "__mouse_studio_v1";
pub const MOUSE: &str = "logitech-mx-anywhere-3s";

pub fn lua_string(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_ascii_control() => out.push_str(&format!("\\{:03}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn shortcut(value: &str) -> Result<(String, String), String> {
    let parts: Vec<_> = value.split('+').map(str::trim).collect();
    let (key, mods) = parts
        .split_last()
        .ok_or("Enter a shortcut, for example CTRL+C.")?;
    let mut modifiers = Vec::new();
    for modifier in mods {
        let canonical = match modifier.to_ascii_uppercase().as_str() {
            "CTRL" | "CONTROL" => "CTRL",
            "SHIFT" => "SHIFT",
            "ALT" => "ALT",
            "SUPER" | "META" => "SUPER",
            _ => return Err("Shortcut modifiers: CTRL, SHIFT, ALT or SUPER.".into()),
        };
        if modifiers.contains(&canonical) {
            return Err("Do not repeat a shortcut modifier.".into());
        }
        modifiers.push(canonical);
    }
    let upper = key.to_ascii_uppercase();
    let function_key = upper
        .strip_prefix('F')
        .and_then(|n| n.parse::<u8>().ok())
        .filter(|n| (1..=35).contains(n));
    let key = if key.len() == 1 && key.chars().all(|c| c.is_ascii_alphanumeric()) {
        key.to_ascii_lowercase()
    } else if let Some(n) = function_key {
        format!("F{n}")
    } else {
        match upper.as_str() {
            "RETURN" | "ENTER" => "Return",
            "ESC" | "ESCAPE" => "Escape",
            "SPACE" => "space",
            "TAB" => "Tab",
            "BACKSPACE" => "BackSpace",
            "DELETE" => "Delete",
            "INSERT" => "Insert",
            "HOME" => "Home",
            "END" => "End",
            "UP" => "Up",
            "DOWN" => "Down",
            "LEFT" => "Left",
            "RIGHT" => "Right",
            "PAGEUP" | "PRIOR" => "Prior",
            "PAGEDOWN" | "NEXT" => "Next",
            "PLUS" => "plus",
            "MINUS" => "minus",
            "EQUAL" => "equal",
            "PRINT" => "Print",
            "XF86AUDIOPLAY" => "XF86AudioPlay",
            "XF86AUDIONEXT" => "XF86AudioNext",
            "XF86AUDIOPREV" => "XF86AudioPrev",
            "XF86AUDIOMUTE" => "XF86AudioMute",
            "XF86AUDIORAISEVOLUME" => "XF86AudioRaiseVolume",
            "XF86AUDIOLOWERVOLUME" => "XF86AudioLowerVolume",
            _ => {
                return Err(
                    "Use a letter, digit, F1–F35, navigation key or supported media key.".into(),
                );
            }
        }
        .into()
    };
    Ok((modifiers.join(" "), key))
}

pub fn dispatcher(binding: &Binding) -> Result<Option<String>, String> {
    if binding.kind == ActionKind::Default {
        return Ok(None);
    }
    if binding.kind == ActionKind::CycleDpi {
        return Ok(Some("function() __mouse_studio_v1.cycle_dpi() end".into()));
    }
    if binding.kind == ActionKind::ActionsRing {
        return Ok(Some("function() __mouse_studio_v1.open_ring() end".into()));
    }
    if binding.value.trim().is_empty() || binding.value.len() > 4096 || binding.value.contains('\0')
    {
        return Err("Enter an action of 1–4096 bytes without null characters.".into());
    }
    Ok(Some(match binding.kind {
        ActionKind::Application | ActionKind::Command => {
            format!("hl.dsp.exec_cmd({})", lua_string(&binding.value))
        }
        ActionKind::Workspace => {
            let n: u8 = binding
                .value
                .trim()
                .parse()
                .map_err(|_| "Workspace must be 1–99.")?;
            if !(1..=99).contains(&n) {
                return Err("Workspace must be 1–99.".into());
            }
            format!("hl.dsp.focus({{workspace={}}})", lua_string(&n.to_string()))
        }
        ActionKind::Shortcut => {
            let (mods, key) = shortcut(&binding.value)?;
            format!(
                "hl.dsp.send_shortcut({{mods={},key={}}})",
                lua_string(&mods),
                lua_string(&key)
            )
        }
        ActionKind::Macro => crate::runtime::macro_expression(&binding.value)?,
        ActionKind::Default => unreachable!(),
        ActionKind::CycleDpi | ActionKind::ActionsRing => unreachable!(),
    }))
}

pub fn token(profile: &Profile) -> String {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_vec(profile)
        .expect("serializable profile")
        .hash(&mut hash);
    format!("{:016x}", hash.finish())
}

pub fn profile_script(profile: &Profile) -> Result<String, String> {
    crate::runtime::profile_script(profile)
}
