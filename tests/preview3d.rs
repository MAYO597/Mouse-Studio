use eframe::egui::*;
use mouse_studio::preview3d::{self, Orbit};
fn frame(ctx: &Context, orbit: &mut Orbit, events: Vec<Event>) {
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            CentralPanel::default().show(ctx, |ui| {
                preview3d::draw(ui, ui.max_rect(), orbit, &[false; 5], &mut 0);
            });
        },
    );
}
#[test]
fn dragging_and_wheel_input_rotate_and_zoom_the_model() {
    let ctx = Context::default();
    let mut orbit = Orbit::default();
    let yaw = orbit.yaw;
    frame(&ctx, &mut orbit, vec![]);
    let start = pos2(150.0, 100.0);
    frame(
        &ctx,
        &mut orbit,
        vec![
            Event::PointerMoved(start),
            Event::PointerButton {
                pos: start,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
    );
    frame(
        &ctx,
        &mut orbit,
        vec![Event::PointerMoved(start + vec2(50.0, 20.0))],
    );
    assert!(orbit.yaw < yaw);
    frame(
        &ctx,
        &mut orbit,
        vec![Event::PointerButton {
            pos: start + vec2(50.0, 20.0),
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
    );
    let zoom = orbit.zoom;
    frame(
        &ctx,
        &mut orbit,
        vec![Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta: vec2(0.0, 100.0),
            modifiers: Modifiers::NONE,
        }],
    );
    assert!(orbit.zoom > zoom);
}

#[test]
fn exported_mouse_mesh_is_finite_and_has_valid_triangles() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/models/mx-anywhere-3s.mesh.json");
    assert!(
        path.exists(),
        "the app needs the actual exported mouse mesh"
    );
    let model: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let parts = model["parts"].as_array().unwrap();
    let hotspots = model["hotspots"].as_array().unwrap();
    assert_eq!(hotspots.len(), 5);
    for (i, name) in ["Back button", "Forward button", "Scroll wheel"]
        .iter()
        .enumerate()
    {
        let part = parts.iter().find(|p| p["name"] == *name).unwrap();
        let at = hotspots[i].as_array().unwrap();
        let distance = part["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                (0..3)
                    .map(|axis| (v[axis].as_f64().unwrap() - at[axis].as_f64().unwrap()).powi(2))
                    .sum::<f64>()
            })
            .fold(f64::INFINITY, f64::min);
        assert!(distance < 0.01, "{name} hotspot drifted off its mesh");
    }
    for name in [
        "Shell left",
        "Shell right",
        "Scroll wheel",
        "Back button",
        "Forward button",
        "Left grip",
        "Base",
    ] {
        assert!(parts.iter().any(|p| p["name"] == name), "missing {name}");
    }
    for part in parts {
        let vertices = part["vertices"].as_array().unwrap();
        let normals = part["normals"].as_array().unwrap();
        assert_eq!(vertices.len(), normals.len());
        for (v, n) in vertices.iter().zip(normals) {
            for c in v.as_array().unwrap() {
                assert!(c.as_f64().unwrap().is_finite());
                assert!(c.as_f64().unwrap().abs() < 2.0);
            }
            let length: f64 = n
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c.as_f64().unwrap().powi(2))
                .sum();
            assert!(
                (length - 1.0).abs() < 0.01,
                "unit surface normals needed for lighting"
            );
        }
        for triangle in part["triangles"].as_array().unwrap() {
            let indices: Vec<usize> = triangle
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i.as_u64().unwrap() as usize)
                .collect();
            assert_eq!(indices.len(), 3);
            assert!(indices.iter().all(|i| *i < vertices.len()));
            let a = vertices[indices[0]].as_array().unwrap();
            let b = vertices[indices[1]].as_array().unwrap();
            let c = vertices[indices[2]].as_array().unwrap();
            let u: Vec<f64> = (0..3)
                .map(|i| b[i].as_f64().unwrap() - a[i].as_f64().unwrap())
                .collect();
            let v: Vec<f64> = (0..3)
                .map(|i| c[i].as_f64().unwrap() - a[i].as_f64().unwrap())
                .collect();
            let area: f64 = (0..3)
                .map(|i| {
                    (u[(i + 1) % 3] * v[(i + 2) % 3] - u[(i + 2) % 3] * v[(i + 1) % 3]).powi(2)
                })
                .sum();
            assert!(area > 1e-16, "degenerate triangle in {}", part["name"]);
        }
    }
}
