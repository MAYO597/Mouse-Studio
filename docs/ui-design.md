# Mouse Studio: Rust UI, first milestone

**Current status:** this document records the initial UI milestone. The user has since approved and locked the [visual baseline](ui-baseline.md). The [four pointer and scrolling controls](device-controls.md) are now connected; their current behavior supersedes the preview-only scope below.

Build a standalone native Rust app for the user's Arch Linux / Hyprland desktop, following the existing Mouse Studio concept and the practical feature list. This milestone is the interface and local profile editing. Mouse configuration is a later integration.

## Design

Update: the user explicitly requested closer matching to the supplied PNG. The current UI uses Inter, Lucide SVG icons, a 1536 × 1024 default window, proportional side panels, a bottom-anchored Settings entry, and inspector-based saving on Overview/Buttons. The original feature scope remains unchanged; unsupported triggers and action testing are visibly disabled. The older palette and initial typography notes below describe the first pass.

Use egui/eframe with its OpenGL renderer and Wayland support. This keeps the application in Rust; Slint would add a declarative UI language, while a Tauri approach would add a web frontend. Pin the tested eframe 0.33 API family.

Preserve the concept's graphite background (#151919), raised surfaces (#1D2221), mint accent (#A6E3CC), off-white text (#EDF2F0), muted text (#A0ACA7), and borders (#333C38). Use restrained rounded controls, a generous device illustration, and a compact native settings layout. egui's bundled sans font handles UI text; its monospace font handles command values.

Three columns: navigation and device status, central settings/device view, button inspector. Navigation: Overview, Buttons, Pointer, Scrolling, Profiles, Settings. At smaller sizes, move the inspector into the scrollable central column. The signature interaction is selecting real mouse buttons through hotspots on an official static product image; front, top and side views are photographs, not simulated 3D.

## Behavior

- Clickable navigation and button selection, with keyboard-accessible standard controls.
- Edit standard button action kinds: default, launch application, shortcut, command, workspace.
- Pointer sensitivity and acceleration, scrolling speed and direction.
- Create, duplicate, rename, delete and manually select local profiles.
- Explicit Save and Discard; local JSON persistence, validation, and import/export paths.
- Read-only Linux input discovery supplies actual connected/disconnected status. No invented battery value.
- Historical: this baseline described the first preview, which did not apply settings. Saving now applies them in Hyprland and can install the optional user service; see the README and `docs/device-controls.md`.

## Structure and validation

Separate profile model and storage, theme/widgets, device discovery, and UI composition. Validate loaded profile names, IDs, active profile, finite numeric ranges and action values. Save atomically; report failures and preserve unsaved edits. Ask before discarding unsaved edits when closing.

Check storage roundtrips and rejection of malformed data, compile and lint the app, exercise navigation and editing through egui input tests, and inspect a native rendered screenshot when the display is accessible.

## Implementation sequence

1. Create Cargo app, local profile model, storage tests, and product assets.
2. Build theme, navigation, device stage, button inspector, and settings pages.
3. Connect local editing, profile operations, persistence and honest device status.
4. Run tests, formatter, clippy, native build and visual verification; document launch instructions.
