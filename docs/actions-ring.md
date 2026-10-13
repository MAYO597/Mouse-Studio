# Actions Ring

A radial menu of 2–8 actions that opens at the mouse cursor when a button is pressed, like Logitech Options+. It is available as the **Actions Ring** action type on any button trigger (Click, Hold, Double-click, gesture directions). It is designed for the MX Master 4 gesture button and Haptic Sense panel button, but the validation does not restrict the device.

## Using it

1. Buttons → choose a control → **Action type → Actions Ring** → **Edit ring**.
2. Add slices (label, action kind, value). Slice 0 is at twelve o'clock; the rest run clockwise. **Load starter ring** inserts eight harmless shortcut/workspace slices to edit. Slice actions can be Application, Shortcut, Command, Workspace, Macro (same line format as the macro editor) or Cycle DPI. A ring cannot contain another ring.
3. The editor shows a live preview; hovering highlights a slice but never runs it. Radius (90–220 px), the neutral centre (10–80 px) and "choose by releasing" are adjustable.
4. **Save profiles**. Saving installs the background helper, as for horizontal scrolling.

At runtime: hold the button, move toward a slice and release to run it. Or tap the button and click a slice. The centre selects nothing. **Esc**, a click outside the disc, or the window losing focus closes it without running anything. Releasing in the centre or outside switches to click mode. Pressing the button while a ring is open does not stack another ring.

## How it works

The runtime's dispatcher for the button runs `mouse-studio --ring <profile-id> <device>` from the installed helper binary (`~/.local/share/mouse-studio/bin/mouse-studio`) through Hyprland's exec dispatcher. That separate process, never the settings window:

1. takes an exclusive lock (`$XDG_RUNTIME_DIR/mouse-studio-ring.lock`) so only one ring exists;
2. reads the **saved** settings file for the device and the profile's ring, validating it (an invalid ring or fewer than two slices exits without a window);
3. opens a frameless, transparent, always-on-top eframe window, app id `mouse-studio-ring`, sized to the ring;
4. asks Hyprland (`hyprctl cursorpos`, then a `pcall`-guarded Lua script) to float, size, move so it is centred on the cursor, and pin the window once it is mapped. Failed steps are printed to stderr and only affect positioning: slices are always selected relative to the window centre, so the ring still works if placement fails;
5. polls the runtime's real button state (about every 40 ms) to detect release-to-select;
6. on a chosen slice, closes the window, waits 150 ms so focus returns to the previous window, then runs the action through the same dispatch as **Test action** (shortcut key taps, `exec_cmd`, workspace focus, macros, Cycle DPI). Nothing runs unless a slice was chosen.

The ring lives in each profile (`advanced.ring`), so it is included in profile duplication/reset (reset clears it), JSON import/export, validation and previous-setup restoration. Old files load with an empty ring. A profile whose buttons open the ring must have at least two valid slices or saving is refused. **Test action** is intentionally unavailable for the ring (press the physical button after saving).

## Verification and limits

Automated (tests/ring.rs and the existing suite): slice geometry for 2–8 slices, centre/outside handling, release/click selection state machine, validation, serde defaults, JSON round trip, previous-setup restore, command-line parsing, cursor parsing, placement script construction, single-instance lock, a Lua mock proving the button launches `--ring <id> <device>` only for the ring button, and the helper requirement.

**Not verified** (the development session had no compositor, GPU or mouse):

- the overlay window was never opened or seen; drawing, transparency and text fitting are unchecked beyond compiling;
- the Hyprland Lua dispatchers used for placement (`hl.dsp.window.float/resize/move/pin`) are written from memory of the 0.5x API and may need adjusting; a brief tiled flash before the window floats is possible;
- whether Hyprland gives the new window keyboard focus and pointer-enter events immediately (Esc and focus-loss close depend on this; a 3 s no-focus timeout and a 60 s hard timeout close it otherwise);
- release-to-select with the physical gesture/panel buttons, ~40 ms polling latency, and shortcuts reaching the previously focused window after the overlay closes;
- multi-monitor/scaled displays: cursor coordinates are used as global layout pixels.
