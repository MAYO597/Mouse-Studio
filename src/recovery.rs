//! One previous saved setup, committed atomically with the current settings.
use crate::model::{Settings, write_settings_file};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read, path::Path};

#[derive(Serialize, Deserialize)]
struct StoredSetup {
    // Existing readers, exports and the background service still see Settings.
    #[serde(flatten)]
    current: Settings,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous_setup: Option<Settings>,
}

fn load(path: &Path) -> Result<Option<StoredSetup>, String> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Cannot read saved setup: {error}")),
    };
    let mut data = Vec::new();
    file.take(2_000_001)
        .read_to_end(&mut data)
        .map_err(|e| format!("Cannot read saved setup: {e}"))?;
    if data.len() > 2_000_000 {
        return Err("Settings file is too large (maximum 2 MB).".into());
    }
    let mut stored: StoredSetup = serde_json::from_slice(&data)
        .map_err(|e| format!("Cannot read saved setup/history: {e}"))?;
    stored.current.validate()?;
    // An unusable restore point must not block saving the valid current setup.
    if stored
        .previous_setup
        .as_ref()
        .is_some_and(|previous| previous.validate().is_err())
    {
        stored.previous_setup = None;
    }
    Ok(Some(stored))
}

pub fn previous_setup(path: &Path) -> Result<Option<Settings>, String> {
    Ok(load(path)?.and_then(|stored| stored.previous_setup))
}

pub fn save_with_history(settings: &Settings, path: &Path) -> Result<(), String> {
    settings.validate()?;
    let previous_setup = load(path)?.and_then(|stored| {
        if stored.current == *settings {
            stored.previous_setup
        } else {
            Some(stored.current)
        }
    });
    let stored = StoredSetup {
        current: settings.clone(),
        previous_setup,
    };
    let data = serde_json::to_vec_pretty(&stored).map_err(|e| e.to_string())?;
    write_settings_file(path, &data)
}
