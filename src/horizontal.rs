//! Private wheel-only IPC; this helper never reads physical input devices.
use evdev::{
    AttributeSet, EventType, InputEvent, KeyCode, RelativeAxisCode, uinput::VirtualDevice,
};
use std::{
    fs,
    os::unix::{
        fs::{FileTypeExt, PermissionsExt},
        net::UnixDatagram,
    },
    path::PathBuf,
};

pub fn socket_path() -> Result<PathBuf, String> {
    socket_path_for(Default::default())
}
pub fn socket_path_for(model: crate::device::MouseModel) -> Result<PathBuf, String> {
    let root = std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR is unavailable.")?;
    let root = PathBuf::from(root);
    if !root.is_absolute() {
        return Err("XDG_RUNTIME_DIR must be absolute.".into());
    }
    Ok(
        root.join(if model == crate::device::MouseModel::MxAnywhere3s {
            "mouse-studio-scroll.sock".into()
        } else {
            format!("mouse-studio-{}-scroll.sock", model.slug())
        }),
    )
}
pub fn validate_request(data: &[u8]) -> Result<i32, String> {
    match data {
        b"1" => Ok(1),
        b"-1" => Ok(-1),
        _ => Err("Horizontal scroll accepts only 1 or -1.".into()),
    }
}
pub fn send(amount: &str) -> Result<(), String> {
    send_for(amount, Default::default())
}
pub fn send_for(amount: &str, model: crate::device::MouseModel) -> Result<(), String> {
    validate_request(amount.as_bytes())?;
    let socket = UnixDatagram::unbound().map_err(|e| e.to_string())?;
    socket
        .send_to(amount.as_bytes(), socket_path_for(model)?)
        .map_err(|e| format!("Horizontal-scroll service unavailable: {e}"))?;
    Ok(())
}
pub fn wait_ready() -> Result<(), String> {
    wait_ready_for(Default::default())
}
pub fn wait_ready_for(model: crate::device::MouseModel) -> Result<(), String> {
    let path = socket_path_for(model)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        if UnixDatagram::unbound()
            .and_then(|s| s.connect(&path))
            .is_ok()
        {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err("Horizontal-scroll helper did not start. Check mouse-studio-restore.service and /dev/uinput access, then Save again.".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
pub fn virtual_device() -> Result<VirtualDevice, String> {
    let axes: AttributeSet<RelativeAxisCode> = [
        RelativeAxisCode::REL_X,
        RelativeAxisCode::REL_Y,
        RelativeAxisCode::REL_HWHEEL,
    ]
    .into_iter()
    .collect();
    // BTN_LEFT + X/Y identify this as a pointer to libinput; only wheel events are emitted.
    let keys: AttributeSet<KeyCode> = [KeyCode::BTN_LEFT].into_iter().collect();
    VirtualDevice::builder()
        .and_then(|b| {
            b.name("Mouse Studio horizontal scroll")
                .with_relative_axes(&axes)
        })
        .and_then(|b| b.with_keys(&keys))
        .and_then(|b| b.build())
        .map_err(|e| format!("Cannot create horizontal-scroll pointer: {e}"))
}
pub fn emit(device: &mut VirtualDevice, amount: i32) -> Result<(), String> {
    if ![-1, 1].contains(&amount) {
        return Err("Invalid wheel step.".into());
    }
    device
        .emit(&[InputEvent::new(
            EventType::RELATIVE.0,
            RelativeAxisCode::REL_HWHEEL.0,
            amount,
        )])
        .map_err(|e| format!("Horizontal scroll failed: {e}"))
}
pub struct Server {
    socket: UnixDatagram,
    device: VirtualDevice,
    path: PathBuf,
}
impl Server {
    pub fn new() -> Result<Self, String> {
        Self::for_device(Default::default())
    }
    pub fn for_device(model: crate::device::MouseModel) -> Result<Self, String> {
        let path = socket_path_for(model)?;
        // Preserve a live socket owned by another instance.
        if fs::symlink_metadata(&path).is_ok() {
            if !fs::symlink_metadata(&path)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_socket()
            {
                return Err(
                    "An unrelated file occupies the scroll socket path; it was preserved.".into(),
                );
            }
            let probe = UnixDatagram::unbound().map_err(|e| e.to_string())?;
            if probe.connect(&path).is_ok() {
                return Err("Horizontal-scroll service is already running.".into());
            }
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        let device = virtual_device()?;
        let socket = UnixDatagram::bind(&path).map_err(|e| e.to_string())?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
        socket.set_nonblocking(true).map_err(|e| e.to_string())?;
        Ok(Self {
            socket,
            device,
            path,
        })
    }
    pub fn pump(&mut self) -> Result<(), String> {
        let mut bytes = [0u8; 8];
        for _ in 0..128 {
            match self.socket.recv(&mut bytes) {
                Ok(n) => {
                    if let Ok(amount) = validate_request(&bytes[..n]) {
                        emit(&mut self.device, amount)?;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(e) => return Err(e.to_string()),
            }
        }
        Ok(())
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
