//! Bolt HID++ routing: identify paired slots before using a shared receiver interface.
use super::{Transport, linux::Hidraw};
use crate::device::MouseModel;
use std::{
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::Instant,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Pairing {
    pub slot: u8,
    pub kind: u8,
    pub model: Option<MouseModel>,
}
pub fn pairing(slot: u8, info: &[u8], name: &[u8]) -> Result<Pairing, String> {
    if !(1..=6).contains(&slot)
        || info.len() < 8
        || info[0] != 0x50 + slot
        || name.len() < 3
        || name[0] != 0x60 + slot
        || name[1] != 1
    {
        return Err("Incomplete Bolt pairing information.".into());
    }
    let len = usize::from(name[2]).min(14);
    let text = std::str::from_utf8(name.get(3..3 + len).ok_or("Incomplete Bolt mouse name.")?)
        .map_err(|_| "Invalid Bolt mouse name.")?;
    Ok(Pairing {
        slot,
        kind: info[1] & 15,
        model: MouseModel::ALL.into_iter().find(|m| m.matches_name(text)),
    })
}
pub fn pointer_name(name: &str) -> bool {
    ["logitech-usb-receiver", "logitech-usb-receiver-mouse"]
        .iter()
        .any(|prefix| {
            name == *prefix
                || name
                    .strip_prefix(&format!("{prefix}-"))
                    .is_some_and(|suffix| {
                        !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit())
                    })
        })
}
pub fn validate_shared_pointer(model: MouseModel, pairs: &[Pairing]) -> Result<(), String> {
    let mice: Vec<_> = pairs
        .iter()
        .filter(|p| matches!(p.kind, 2 | 8 | 9))
        .collect();
    match mice.as_slice() {
        [p] if p.model==Some(model)=>Ok(()),
        [_]=>Err("This receiver is paired with another mouse model.".into()),
        []=>Err("No mouse is paired with this Bolt receiver.".into()),
        _=>Err("This receiver has multiple paired pointing devices. Use a separate receiver or Bluetooth to keep mouse settings isolated.".into()),
    }
}
fn paths() -> Result<Vec<PathBuf>, String> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir("/sys/class/hidraw")
        .map_err(|e| e.to_string())?
        .flatten()
    {
        let Ok(u) = std::fs::read_to_string(entry.path().join("device/uevent")) else {
            continue;
        };
        if u.lines().any(|l| l == "HID_ID=0003:0000046D:0000C548")
            && u.lines()
                .any(|l| l.starts_with("HID_PHYS=") && l.ends_with("/input2"))
        {
            found.push(PathBuf::from("/dev").join(entry.file_name()));
        }
    }
    Ok(found)
}
fn enumerate(wire: &mut Hidraw) -> Result<Vec<Pairing>, String> {
    let mut pairs = Vec::new();
    for slot in 1..=6 {
        if let Some(info) = wire.receiver_info(0x50 + slot, 0)? {
            let name = wire
                .receiver_info(0x60 + slot, 1)?
                .ok_or("Missing Bolt device name.")?;
            pairs.push(pairing(slot, &info, &name)?);
        }
    }
    Ok(pairs)
}
type Seen = Vec<(MouseModel, PathBuf, Instant)>;
fn seen() -> &'static Mutex<Seen> {
    static SEEN: OnceLock<Mutex<Seen>> = OnceLock::new();
    SEEN.get_or_init(Mutex::default)
}
pub fn known_live(model: MouseModel) -> bool {
    seen()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .any(|(m, path, time)| *m == model && time.elapsed().as_secs() < 10 && path.exists())
}
pub(super) fn open(model: MouseModel, shared_pointer: bool) -> Result<Hidraw, String> {
    let devices = paths()?;
    let [path] = devices.as_slice() else {
        return Err(if devices.is_empty() {
            "Bluetooth mouse or Bolt receiver is not connected."
        } else {
            "Multiple Bolt receivers are connected; receiver routing is ambiguous."
        }
        .into());
    };
    if shared_pointer {
        // Generic compositor names cannot distinguish another generic Logitech receiver.
        let data = std::fs::read_to_string("/proc/bus/input/devices").map_err(|e| e.to_string())?;
        for block in data.split("\n\n") {
            if block.contains("N: Name=\"Logitech USB Receiver")
                && !block.contains("Vendor=046d Product=c548")
            {
                return Err(
                    "Another generic Logitech receiver is connected; mouse routing is ambiguous."
                        .into(),
                );
            }
        }
    }
    let mut wire = Hidraw::open_path(path, 0xff)?;
    let pairs = enumerate(&mut wire)?;
    if shared_pointer {
        validate_shared_pointer(model, &pairs)?;
    }
    let matching: Vec<_> = pairs
        .iter()
        .filter(|p| p.model == Some(model) && p.kind == 2)
        .collect();
    let [pair] = matching.as_slice() else {
        return Err(format!(
            "Expected one paired {} on the Bolt receiver.",
            model.name()
        ));
    };
    wire.address = pair.slot;
    // A receiver mouse may wake only after the first query. Retry this read-only
    // ping once; setting writes are never retried blindly.
    let ping = wire.request(0, 0x10, &[0, 0, 0x5a]).or_else(|error| {
        if error.contains("timed out") {
            wire.request(0, 0x10, &[0, 0, 0x5a])
        } else {
            Err(error)
        }
    })?;
    if ping.len() < 3 || ping[0] < 2 || ping[2] != 0x5a {
        return Err(
            "The paired mouse did not answer a valid HID++ ping. Wake it and retry.".into(),
        );
    }
    // Pairing metadata can be stale. Confirm the live device name before any control write.
    let index = wire
        .request(0, 0, &[0, 5])?
        .first()
        .copied()
        .filter(|i| *i != 0)
        .ok_or("Cannot verify the live receiver mouse name.")?;
    let len = wire
        .request(index, 0, &[])?
        .first()
        .copied()
        .filter(|n| *n > 0 && *n <= 80)
        .ok_or("Invalid live mouse name length.")? as usize;
    let mut name = Vec::new();
    while name.len() < len {
        let fragment = wire.request(index, 0x10, &[name.len() as u8])?;
        if fragment.is_empty() {
            return Err("Incomplete live mouse name.".into());
        }
        name.extend_from_slice(&fragment[..fragment.len().min(len - name.len())]);
    }
    if !std::str::from_utf8(&name).is_ok_and(|s| model.matches_name(s)) {
        return Err("Live mouse identity does not match the selected model.".into());
    }
    let mut cache = seen().lock().unwrap_or_else(|e| e.into_inner());
    cache.retain(|(m, _, _)| *m != model);
    cache.push((model, path.clone(), Instant::now()));
    Ok(wire)
}
pub fn verify_pointer(model: MouseModel) -> Result<(), String> {
    open(model, true).map(|_| ())
}

/// Read-only startup detection. Pairing metadata alone does not prove a mouse is live.
pub fn connected_model() -> Option<MouseModel> {
    let devices = paths().ok()?;
    let [path] = devices.as_slice() else {
        return None;
    };
    let mut wire = Hidraw::open_path(path, 0xff).ok()?;
    let pairs = enumerate(&mut wire).ok()?;
    // Release the request lock before reusing the existing live-identity checks.
    drop(wire);
    connected_model_from(&pairs, |model| open(model, false).is_ok())
}

pub fn connected_model_from(
    pairs: &[Pairing],
    mut verify_live: impl FnMut(MouseModel) -> bool,
) -> Option<MouseModel> {
    MouseModel::ALL.into_iter().find(|model| {
        pairs
            .iter()
            .any(|pair| pair.kind == 2 && pair.model == Some(*model))
            && verify_live(*model)
    })
}
