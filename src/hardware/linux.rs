use super::{Battery, Client, HardwareSettings, Snapshot, Transport, match_reply};
use crate::device::MouseModel;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
fn device_path(model: MouseModel) -> Result<Option<PathBuf>, String> {
    let entries = fs::read_dir("/sys/class/hidraw").map_err(|e| e.to_string())?;
    let mut matches = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        let Ok(u) = fs::read_to_string(p.join("device/uevent")) else {
            continue;
        };
        if bluetooth_matches(&u, model) {
            matches.push(PathBuf::from("/dev").join(entry.file_name()));
        }
    }
    match matches.as_slice() {
        [p] => Ok(Some(p.clone())),
        [] => Ok(None),
        _ => Err(format!(
            "Multiple {} mice are connected; hardware control is ambiguous.",
            model.name()
        )),
    }
}
pub fn identity_for(model: MouseModel) -> Option<String> {
    let p = match device_path(model).ok()? {
        Some(p) => p,
        None => {
            let wire = super::receiver::open(model, false).ok()?;
            return Some(format!("{}:slot{}", wire.identity, wire.address));
        }
    };
    fs::canonicalize(
        Path::new("/sys/class/hidraw")
            .join(p.file_name()?)
            .join("device"),
    )
    .ok()
    .map(|p| p.display().to_string())
}
pub(super) struct Hidraw {
    file: File,
    software_id: u8,
    pub(super) address: u8,
    pub(super) identity: String,
    pub(super) path: PathBuf,
}
impl Hidraw {
    pub(super) fn open(model: MouseModel) -> Result<Self, String> {
        if let Some(path) = device_path(model)? {
            Self::open_path(&path, 0xff)
        } else {
            super::receiver::open(model, false)
        }
    }
    pub(super) fn open_path(p: &Path, address: u8) -> Result<Self, String> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
            .open(p)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    "Mouse hardware access needs the device-access script in Settings.".into()
                } else {
                    format!("Cannot open mouse HID interface: {e}")
                }
            })?;
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            // Advisory lock coordinates our independent GUI/service/helper descriptors.
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                break;
            }
            let e = std::io::Error::last_os_error();
            if e.raw_os_error() != Some(libc::EWOULDBLOCK) || Instant::now() >= deadline {
                return Err(format!("Mouse hardware is busy: {e}"));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(Self {
            file,
            path: p.to_path_buf(),
            software_id: 8,
            address,
            identity: fs::canonicalize(
                Path::new("/sys/class/hidraw")
                    .join(p.file_name().ok_or("Invalid HID path")?)
                    .join("device"),
            )
            .map_err(|e| e.to_string())?
            .display()
            .to_string(),
        })
    }
}
impl Hidraw {
    pub(super) fn event_reader(&self) -> Result<File, String> {
        // A separate descriptor receives its own copy of notifications. It does
        // not retain the request lock, so battery/DPI operations can continue.
        OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
            .open(&self.path)
            .map_err(|e| format!("Cannot listen for extra buttons: {e}"))
    }
    pub(super) fn receiver_info(&mut self, sub: u8, page: u8) -> Result<Option<Vec<u8>>, String> {
        let mut buffer = [0u8; 64];
        for _ in 0..256 {
            match self.file.read(&mut buffer) {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.to_string()),
            }
        }
        self.file
            .write_all(&[0x10, 0xff, 0x83, 0xb5, sub, page, 0])
            .map_err(|e| e.to_string())?;
        let deadline = Instant::now() + Duration::from_millis(750);
        while Instant::now() < deadline {
            let mut fd = libc::pollfd {
                fd: self.file.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let ready = unsafe {
                libc::poll(
                    &mut fd,
                    1,
                    deadline
                        .saturating_duration_since(Instant::now())
                        .as_millis() as i32,
                )
            };
            if ready < 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            if fd.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
                return Err("Bolt receiver disconnected.".into());
            }
            if ready == 0 {
                break;
            }
            match self.file.read(&mut buffer) {
                Ok(n) if n >= 7 && buffer[1] == 0xff => {
                    if buffer[2] == 0x8f && buffer[3] == 0x83 && buffer[4] == 0xb5 {
                        return if matches!(buffer[5], 2 | 8) {
                            Ok(None)
                        } else {
                            Err(format!("Bolt pairing query rejected ({}).", buffer[5]))
                        };
                    }
                    if buffer[2] == 0x83 && buffer[3] == 0xb5 && buffer[4] == sub {
                        return Ok(Some(buffer[4..n].to_vec()));
                    }
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("Bolt pairing query timed out.".into())
    }
}
impl Transport for Hidraw {
    fn request(&mut self, feature: u8, function: u8, params: &[u8]) -> Result<Vec<u8>, String> {
        if params.len() > 16 || function & 0x0f != 0 {
            return Err("Invalid HID++ request.".into());
        }
        let function = function | self.software_id;
        self.software_id = 8 + (self.software_id + 1) % 8;
        // Drain this descriptor's old reports. Kernel input remains independent.
        let mut buffer = [0; 64];
        for _ in 0..256 {
            match self.file.read(&mut buffer) {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(format!("Mouse disconnected: {e}")),
            }
        }
        let mut request = [0u8; 20];
        request[..4].copy_from_slice(&[0x11, self.address, feature, function]);
        request[4..4 + params.len()].copy_from_slice(params);
        self.file
            .write_all(&request)
            .map_err(|e| format!("Mouse HID write failed: {e}"))?;
        let deadline = Instant::now() + Duration::from_millis(750);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("Mouse HID++ request timed out. Wake or reconnect the mouse.".into());
            }
            let mut fd = libc::pollfd {
                fd: self.file.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let ready = unsafe {
                libc::poll(
                    &mut fd,
                    1,
                    remaining.as_millis().min(i32::MAX as u128) as i32,
                )
            };
            if ready < 0 {
                let e = std::io::Error::last_os_error();
                if e.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e.to_string());
            }
            if fd.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
                return Err("Mouse HID interface disconnected.".into());
            }
            if ready == 0 {
                continue;
            }
            match self.file.read(&mut buffer) {
                Ok(0) => return Err("Mouse disconnected.".into()),
                Ok(n) => {
                    if let Some(reply) = match_reply(&buffer[..n], self.address, feature, function)?
                    {
                        return Ok(reply);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e.to_string()),
            }
        }
    }
}
pub fn read_battery_for(root: &Path, model: MouseModel) -> Option<Battery> {
    let mut found = Vec::new();
    for entry in fs::read_dir(root).ok()?.flatten() {
        let p = entry.path();
        if !fs::read_to_string(p.join("model_name")).is_ok_and(|s| model.matches_name(s.trim())) {
            continue;
        }
        let percent = fs::read_to_string(p.join("capacity"))
            .ok()
            .and_then(|s| s.trim().parse::<u8>().ok())
            .filter(|p| *p <= 100);
        let status = fs::read_to_string(p.join("status"))
            .ok()
            .map(|s| s.trim().to_string())
            .unwrap_or("Unknown".into());
        found.push(Battery { percent, status });
    }
    if found.len() == 1 { found.pop() } else { None }
}
pub fn snapshot_for(model: MouseModel) -> Snapshot {
    let battery = read_battery_for(Path::new("/sys/class/power_supply"), model);
    match Hidraw::open(model).and_then(|w| {
        let mut client = Client::new(w);
        let mut snapshot = client.read()?;
        snapshot.battery = client.battery().ok().flatten();
        // Read-only and optional: a failed haptic query only hides those controls.
        snapshot.haptics = client.read_haptics().ok().flatten();
        Ok(snapshot)
    }) {
        Ok(mut s) => {
            s.battery = s.battery.or(battery);
            s
        }
        Err(e) => Snapshot {
            battery,
            error: Some(e),
            ..Default::default()
        },
    }
}
pub fn apply_for(settings: &HardwareSettings, model: MouseModel) -> Result<(), String> {
    if !settings.managed() {
        return Ok(());
    }
    Client::new(Hidraw::open(model)?).apply(settings)
}
pub fn play_haptic_for(model: MouseModel) -> Result<(), String> {
    if model != MouseModel::MxMaster4 {
        return Err("Haptic feedback belongs to the MX Master 4.".into());
    }
    Client::new(Hidraw::open(model)?).play_haptic()
}
pub fn cycle_for(presets: &[u16], model: MouseModel) -> Result<(), String> {
    Client::new(Hidraw::open(model)?).cycle(presets)
}
pub fn toggle_wheel_for(model: MouseModel) -> Result<(), String> {
    let mut client = Client::new(Hidraw::open(model)?);
    let current = client
        .read()?
        .wheel_mode
        .ok_or("Mouse does not support wheel-mode switching.")?;
    client.apply(&HardwareSettings {
        wheel_mode: Some(match current {
            super::WheelMode::Ratchet => super::WheelMode::FreeSpin,
            super::WheelMode::FreeSpin => super::WheelMode::Ratchet,
        }),
        ..Default::default()
    })
}

pub fn bluetooth_matches(uevent: &str, model: MouseModel) -> bool {
    uevent.lines().any(|l| {
        l.strip_prefix("HID_NAME=")
            .is_some_and(|n| model.matches_name(n))
    }) && uevent
        .lines()
        .any(|l| l.starts_with("HID_ID=0005:0000046D:"))
}
pub fn identity() -> Option<String> {
    identity_for(Default::default())
}
pub fn read_battery(root: &Path) -> Option<Battery> {
    read_battery_for(root, Default::default())
}
pub fn snapshot() -> Snapshot {
    snapshot_for(Default::default())
}
pub fn apply(settings: &HardwareSettings) -> Result<(), String> {
    apply_for(settings, Default::default())
}
pub fn cycle(presets: &[u16]) -> Result<(), String> {
    cycle_for(presets, Default::default())
}
