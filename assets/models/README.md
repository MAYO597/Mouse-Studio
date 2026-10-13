# MX Anywhere 3S — 3D model

A photo-referenced graphite mouse model made for Mouse Studio. This is an artistic approximation for visualization, not manufacturer CAD or a fabrication model.

- **mx-anywhere-3s.blend** — editable Blender scene with 42 named components, materials, product camera and studio lights.
- **mx-anywhere-3s.glb** — portable glTF binary with geometry and materials. Coordinates use meters; overall length is approximately 100.5 mm.
- **mx-anywhere-3s-preview.png**, **-top.png**, **-side.png**, **-back.png** — four rendered views.
- **reference-comparison.html** — the four supplied PNGs beside the corresponding renders.
- **mx-anywhere-3s.mesh.json** — the same geometry, exported in the normalized Y-up coordinates used by the Rust app. It includes vertex normals and material colors.

Details include the contoured shell, center panel, metal wheel with toothed bands, mode button, green indicator lens matching the reference appearance, separate Back/Forward buttons, ribbed silicone grips, USB-C opening, underside sensor, switch and glide feet. Small underside details and dimensions are approximate. The detailed mesh contains approximately 136,000 triangles; no external textures or network requests are required.

In Mouse Studio, select **3D**, drag to rotate and scroll to zoom. Numbered markers select Back, Forward and Middle. Occluded markers disappear. Live presses still use the existing monitoring system. The app embeds the mesh at build time and caches a shaded image with per-pixel depth; it redraws the geometry only when the view or canvas dimensions change. Existing photograph views remain available.

To regenerate all assets from the project directory:

```bash
blender --background --factory-startup --python scripts/build_mouse_model.py
cargo build --release
```

The generator consumes `scripts/mouse-reference.json`, which contains the measured top outline and logo silhouette extracted from the supplied top PNG. Crown and heel sections, grip sculpting and control geometry are in the Python generator. Button hotspot positions are exported with the model to keep the app aligned.

The green lens is part of the model’s appearance and does not report battery or connection status.

The generator replaces these model files. Preserve manual Blender edits under a different filename before regenerating. Blender is required only for editing/regeneration; the app needs no Blender installation.

Reference images: the existing `assets/mouse-3qtr-front.png`, `assets/mouse-top-view.png` and `assets/mouse-side-left.png` and `assets/mouse-3qtr-back.png`. The mesh was authored procedurally from those visual references; it was not downloaded from a third-party model library.
