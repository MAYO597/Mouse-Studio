//! Opt-in per-user startup installation and headless profile restoration.
use crate::{
    backend::{Hyprctl, Hyprland},
    model::{Profile, Settings},
};
use std::{
    ffi::OsStr,
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::Duration,
};

const SERVICE: &str = "mouse-studio-restore.service";
pub fn service_name(model: crate::device::MouseModel) -> String {
    if model == crate::device::MouseModel::MxAnywhere3s {
        SERVICE.into()
    } else {
        format!("mouse-studio-{}-restore.service", model.slug())
    }
}
const OWNER: &str = "# Managed by Mouse Studio\n";
static STOPPING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
extern "C" fn stop_service(_: libc::c_int) {
    STOPPING.store(true, std::sync::atomic::Ordering::Relaxed);
}

pub trait StartupControl: Send + Sync {
    fn sync(&self, enabled: bool, settings: &Path) -> Result<(), String>;
}

pub trait ServiceRunner: Send + Sync {
    fn run(&self, args: &[&str]) -> Result<(), String>;
}

pub struct Systemctl;
impl ServiceRunner for Systemctl {
    fn run(&self, args: &[&str]) -> Result<(), String> {
        let mut all = vec![OsStr::new("--user")];
        all.extend(args.iter().map(OsStr::new));
        // A restart/disable may wait for the unit's 30-second button cleanup.
        crate::process::run(OsStr::new("systemctl"), &all, Duration::from_secs(45)).map(|_| ())
    }
}

pub struct Startup<R = Systemctl> {
    pub config_root: PathBuf,
    pub data_root: PathBuf,
    pub source_binary: PathBuf,
    pub runner: R,
}

fn directory(variable: &str, fallback: &str) -> Result<PathBuf, String> {
    if let Some(value) = std::env::var_os(variable) {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            return Ok(path);
        }
    }
    let home = std::env::var_os("HOME").ok_or("Cannot locate your home directory.")?;
    Ok(PathBuf::from(home).join(fallback))
}

pub fn scroll_helper_command() -> Result<String, String> {
    let binary = directory("XDG_DATA_HOME", ".local/share")?.join("mouse-studio/bin/mouse-studio");
    let path = binary.to_str().ok_or("Helper path is not UTF-8")?;
    Ok(format!("'{}'", path.replace('\'', "'\\''")))
}

impl Startup {
    pub fn from_environment() -> Result<Self, String> {
        Ok(Self {
            config_root: directory("XDG_CONFIG_HOME", ".config")?,
            data_root: directory("XDG_DATA_HOME", ".local/share")?,
            source_binary: {
                // A replaced binary reads back as "<path> (deleted)".
                let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                match exe.to_str().and_then(|s| s.strip_suffix(" (deleted)")) {
                    Some(trimmed) => std::path::PathBuf::from(trimmed),
                    None => exe,
                }
            },
            runner: Systemctl,
        })
    }
}

fn systemd_word(path: &Path) -> Result<String, String> {
    let path = path.to_str().ok_or("Startup paths must be valid UTF-8.")?;
    if path.chars().any(char::is_control) {
        return Err("Startup paths cannot contain control characters.".into());
    }
    Ok(format!(
        "\"{}\"",
        path.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
            .replace('$', "$$")
    ))
}

pub fn unit_text(binary: &Path, settings: &Path) -> Result<String, String> {
    if !binary.is_absolute() || !settings.is_absolute() {
        return Err("Startup paths must be absolute.".into());
    }
    Ok(format!(
        "{OWNER}[Unit]\nDescription=Restore Mouse Studio profiles in Hyprland\n\n[Service]\nType=simple\nExecStart={} --restore-service --settings {}\nRestart=on-failure\nRestartSec=5\nTimeoutStopSec=30\n\n[Install]\nWantedBy=default.target\n",
        systemd_word(binary)?,
        systemd_word(settings)?
    ))
}

fn write_atomic(path: &Path, bytes: &[u8], mode: u32) -> Result<(), String> {
    let parent = path.parent().ok_or("Invalid install path")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())?;
    file.as_file()
        .set_permissions(fs::Permissions::from_mode(mode))
        .map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

impl<R: ServiceRunner> StartupControl for Startup<R> {
    fn sync(&self, enabled: bool, settings: &Path) -> Result<(), String> {
        let model = if settings.exists() {
            Settings::load_from(settings)?.device
        } else {
            Default::default()
        };
        let service = service_name(model);
        let unit = self.config_root.join("systemd/user").join(&service);
        let previous = match fs::read_to_string(&unit) {
            Ok(text) => {
                if !text.starts_with(OWNER) {
                    return Err("An unrelated mouse-studio-restore.service already exists; it was not changed.".into());
                }
                Some(text)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        if !enabled {
            if previous.is_some() {
                self.runner.run(&["disable", "--now", &service])?;
                fs::remove_file(&unit).map_err(|e| e.to_string())?;
                self.runner.run(&["daemon-reload"])?;
            }
            return Ok(());
        }
        let binary = self.data_root.join("mouse-studio/bin/mouse-studio");
        let text = unit_text(&binary, settings)?;
        let text = if model == crate::device::MouseModel::MxAnywhere3s {
            text
        } else {
            text.replace(
                " --restore-service",
                &format!(" --device {} --restore-service", model.slug()),
            )
        };
        let bytes = fs::read(&self.source_binary)
            .map_err(|e| format!("Cannot install restore helper: {e}"))?;
        let binary_changed = fs::read(&binary).ok().as_deref() != Some(bytes.as_slice());
        if binary_changed {
            write_atomic(&binary, &bytes, 0o755)?;
        }
        let unit_changed = previous.as_deref() != Some(&text);
        if unit_changed {
            write_atomic(&unit, text.as_bytes(), 0o644)?;
        }
        self.runner.run(&["daemon-reload"])?;
        self.runner.run(&["enable", "--now", &service])?;
        if (binary_changed || unit_changed) && previous.is_some() {
            self.runner.run(&["restart", &service])?;
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct RestoreLoop {
    last_error: Option<String>,
}
impl RestoreLoop {
    pub fn step(
        &mut self,
        path: &Path,
        apply: impl FnOnce(&Profile) -> Result<bool, String>,
    ) -> Result<bool, String> {
        let result = Settings::load_from(path).and_then(|settings| {
            if settings.restore_on_login {
                apply(settings.active())
            } else {
                Ok(false)
            }
        });
        self.last_error = result.as_ref().err().cloned();
        result
    }
}

pub fn run(path: &Path) -> ! {
    run_for(path, Default::default())
}
pub fn run_for(path: &Path, model: crate::device::MouseModel) -> ! {
    // Let the listener release transient diversions before systemd stops it.
    unsafe {
        libc::signal(
            libc::SIGTERM,
            stop_service as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGINT,
            stop_service as *const () as libc::sighandler_t,
        );
    }
    crate::hardware::desktop::follow_in_background(path.to_path_buf(), model);
    // A service may outlive a compositor. Rediscover the sole session instead of
    // retaining a stale inherited HYPRLAND_INSTANCE_SIGNATURE after restart.
    let backend = Hyprland::with_runner(Hyprctl, None, None).for_device(model);
    let mut error = None;
    let mut scroll: Option<crate::horizontal::Server> = None;
    let mut buttons = (model == crate::device::MouseModel::MxMaster4)
        .then(crate::hardware::buttons::Service::default);
    let mut button_next = std::time::Instant::now();
    let mut button_error = None;
    let mut next = std::time::Instant::now();
    loop {
        if STOPPING.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        if std::time::Instant::now() >= next {
            next = std::time::Instant::now() + Duration::from_secs(5);
            let result = Settings::load_from(path).and_then(|settings| {
                if settings.horizontal_required() {
                    if scroll.is_none() {
                        scroll = Some(crate::horizontal::Server::for_device(model)?);
                    }
                } else {
                    scroll = None;
                }
                if settings.service_required() {
                    backend.ensure_settings(&settings)
                } else {
                    Ok(false)
                }
            });
            if let Err(e) = &result
                && error.as_ref() != Some(e)
            {
                eprintln!("Mouse Studio: {e}");
            }
            error = result.err();
        }
        if STOPPING.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        if let Some(server) = &mut scroll
            && let Err(e) = server.pump()
        {
            eprintln!("Mouse Studio: {e}");
            scroll = None;
        }
        if let Some(service) = &mut buttons {
            let result = if std::time::Instant::now() >= button_next {
                button_next = std::time::Instant::now() + Duration::from_millis(500);
                Settings::load_from(path).and_then(|settings| service.refresh(settings))
            } else {
                Ok(())
            }
            .and_then(|()| service.pump());
            if STOPPING.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            if let Err(e) = &result {
                service.fail(e);
                if button_error.as_ref() != Some(e) {
                    eprintln!("Mouse Studio extra buttons: {e}");
                }
                button_next = std::time::Instant::now() + Duration::from_secs(5);
            }
            if result.is_err() {
                button_error = result.err();
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(buttons);
    drop(scroll);
    let _ = crate::backend::DeviceBackend::cancel_actions(&backend);
    std::process::exit(0);
}
