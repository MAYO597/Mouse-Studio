# Button actions and automatic restore implementation plan

**Goal:** Make the existing button inspector functional and provide optional automatic profile restoration while preserving the approved UI.

**Architecture:** Rust validates actions and generates device-filtered Hyprland Lua bindings. A headless mode of the same binary restores profiles; an optional per-user systemd service runs a stable installed copy. The UI remains an explicit Save/Discard editor.

**Constraints:** No root, logid/config rewrites, external messages, fabricated hardware capabilities, or redesign. Existing preview-era command assignments must not execute on opening/saving; execution happens only on a button press or explicit Test action. The restore service defaults off for old files.

## Deliverables

- [x] Add action validation and Lua generation for applications/commands, shortcuts and workspaces. Escape values as data. Use exact device filtering and owned, reusable bind handles. Never call broad unbind/remove APIs. Default disables only the owned binding.
- [x] Apply pointer settings and bindings in the same backend operation; keep isolated UI tests. Add an explicit action test and error reporting. Preserve selected-profile/save semantics.
- [x] Add backward-compatible `restore_on_login` setting, Settings toggle, stable binary installation and owned user-service enable/disable. No automatic installation merely from opening the app.
- [x] Add headless restoration with validated file reads and retries after missing device/session, reconnect, reload or service restart. Avoid replaying action commands during restoration and avoid duplicate bindings.
- [x] Test validation/escaping, action mapping, binding ownership/idempotence, persistence/backward compatibility, service paths with spaces, restore retries, and failed operations.
- [x] Perform isolated live checks with harmless temporary actions, verify real bind registration/dispatch and service operation, restore previous state, build release, inspect native screenshots, update the feature list and session notes.

## Decisions

- Bindings target only `logitech-mx-anywhere-3s`; Back=275, Forward=276, middle=274. Primary clicks remain untouched.
- On this Hyprland version, a keybind object's remove() removes all matching key/modifier binds. Use set_enabled(false) and retain/reuse the three owned handles instead.
- Shortcut syntax is a validated modifier list plus an XKB key name, e.g. CTRL+SHIFT+C. Application/command strings intentionally use Hyprland's exec dispatcher; their bytes must not escape into Lua.
- Automatic restore is opt-in through Settings and Save profiles. Disabling stops/removes only Mouse Studio's owned service; current mouse settings remain in place.
- Service stores no alternate profile file. It restores the last explicitly saved active profile. Polling checks a compositor marker and device identity; Lua verifies that owned handles still exist after config reload.
- Run implementation inline; no subagents or Git repository initialization required.
