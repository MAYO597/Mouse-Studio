# Hardware controls implementation plan

Goal: implement DPI/presets/cycling, wheel modes, SmartShift and battery status in the existing Rust app.

1. Add failing protocol/battery/profile-compatibility tests. Implement isolated `hardware` module (schema, HID++ client, Linux discovery, transactional application) plus `libc` polling/locking.
2. Add a real-desktop backend wrapper and helper CLI. Extend runtime cycle action and owned service to follow the actually active application profile; retain isolated captures and existing backend tests.
3. Add Pointer/Scrolling controls and Overview battery/readback, preserving draft/Save behavior and reporting permission/missing-feature errors.
4. Run fixture tests, full suite, Clippy, release build, app-only native capture. Probe actual HID++ capabilities and reversible writes when device access is available. Update checklist, README and session with measured results and remaining verification limits.

Review focus: stale telemetry after disconnect; old JSON must leave hardware unmanaged; unsupported DPI must not cause partial writes; application profiles must follow focus while GUI is closed; cycle/restore/poll requests must not race or overwrite each other.
