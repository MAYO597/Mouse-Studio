# Software Features Implementation Plan

**Goal:** Implement all remaining features that do not require HID++ verification.
**Architecture:** Backward-compatible settings feed one device-filtered Lua runtime. The existing user service provides virtual horizontal scrolling; the egui frontend configures and displays that runtime.
**Tech stack:** Rust, egui/eframe, serde, Hyprland Lua, evdev uinput.
**Spec:** ../specs/2026-10-08-software-features-design.md

- [x] Extend profile/settings schema and validation; test legacy JSON, reset identity, app matching and invalid macros/modes.
- [x] Generate/install the shared Lua runtime; deterministic Lua tests cover click/hold/double, direction thresholds, precision release, focus fallback, cancellation and repeat-save handle reuse.
- [x] Implement private horizontal-scroll IPC and virtual axis output; update the service and CLI. Test request validation and service decisions.
- [x] Wire full settings application, action testing/cancellation and asynchronous live telemetry through DeviceBackend.
- [x] Add profile reset, class matching, trigger tabs, advanced modes, macro composition/testing/cancel and gesture test UI.
- [x] Add schematic procedural 3D mesh with orbit/zoom/hotspots while preserving default photos.
- [x] Run automated checks, real Lua/API validation and virtual-device integration, build release, inspect app-only screenshots and update feature/session documentation.

Review focus: malformed legacy/imported data must not panic; focus changes must clear precision/pending actions; cancellation must leave no key down; other mice/user binds must stay untouched; helper failure must be visible rather than reported as successful scrolling.
