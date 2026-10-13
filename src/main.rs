use eframe::egui;
use mouse_studio::{
    app::Studio,
    model::settings_path,
    startup::{self, Startup},
};
use std::sync::Arc;

fn main() -> eframe::Result {
    let mut args: Vec<_> = std::env::args_os().skip(1).collect();
    let requested_model = if let Some(index) = args.iter().position(|a| a == "--device") {
        let value = args
            .get(index + 1)
            .and_then(|s| s.to_str())
            .ok_or_else(|| eframe::Error::AppCreation("Missing --device model".into()))?;
        let model = mouse_studio::device::MouseModel::parse(value)
            .map_err(|e| eframe::Error::AppCreation(e.into()))?;
        args.drain(index..=index + 1);
        Some(model)
    } else {
        None
    };
    // Background services and command helpers retain their original default target.
    let model = requested_model.unwrap_or_default();
    if args.first().is_some_and(|a| a == "--cycle-dpi") {
        let result = (|| {
            if args.len() != 2 {
                return Err("Usage: mouse-studio --cycle-dpi 800,1600,3200".into());
            }
            let values = args[1]
                .to_str()
                .ok_or("Invalid DPI values")?
                .split(',')
                .map(|s| {
                    s.parse::<u16>()
                        .map_err(|_| "Invalid DPI value".to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            mouse_studio::hardware::cycle_for(&values, model)
        })();
        mouse_studio::hardware::desktop::report_action_for(&result, model);
        if let Err(e) = result {
            eprintln!("{e}");
            std::process::exit(1)
        }
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "--ring") {
        let rest: Vec<String> = args[1..]
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        std::process::exit(mouse_studio::ring_ui::run_overlay(&rest));
    }
    if args.first().is_some_and(|a| a == "--hardware-status") && args.len() == 1 {
        let s = mouse_studio::hardware::snapshot_for(model);
        println!("{s:#?}");
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "--toggle-wheel") && args.len() == 1 {
        let result = mouse_studio::hardware::toggle_wheel_for(model);
        mouse_studio::hardware::desktop::report_action_for(&result, model);
        if let Err(e) = result {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "--horizontal-scroll") {
        let result = if args.len() == 2 {
            args[1]
                .to_str()
                .ok_or("Invalid wheel step".into())
                .and_then(|amount| mouse_studio::horizontal::send_for(amount, model))
        } else {
            Err("Usage: mouse-studio --horizontal-scroll 1|-1".into())
        };
        if let Err(e) = result {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return Ok(());
    }
    if args.first().is_some_and(|a| a == "--restore-service") {
        if args.len() != 3 || args[1] != "--settings" {
            eprintln!("Usage: mouse-studio --restore-service --settings /absolute/settings.json");
            std::process::exit(2);
        }
        let path = std::path::PathBuf::from(&args[2]);
        if !path.is_absolute() {
            eprintln!("Settings path must be absolute.");
            std::process::exit(2);
        }
        startup::run_for(&path, model);
    }
    if !args.is_empty() {
        eprintln!("Usage: mouse-studio [--restore-service --settings /absolute/settings.json]");
        std::process::exit(2);
    }
    let model = mouse_studio::device::startup_model(requested_model);
    let startup = Startup::from_environment()
        .map_err(|e| eframe::Error::AppCreation(Box::new(std::io::Error::other(e))))?;
    let path = settings_path()
        .map_err(|e| eframe::Error::AppCreation(Box::new(std::io::Error::other(e))))?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Mouse Studio")
            .with_app_id("mouse-studio")
            .with_inner_size([1536.0, 1024.0])
            .with_min_inner_size([480.0, 480.0])
            .with_decorations(false),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Mouse Studio",
        options,
        Box::new(move |cc| {
            Ok(Box::new(Studio::with_devices(
                &cc.egui_ctx,
                path,
                model,
                Arc::new(startup),
            )))
        }),
    )
}
