# Using Mouse Studio

The detailed behaviour guide: how profiles, button actions, restore and the settings files work.
For what the app does and how to install it, see the [README](../README.md). The original, longer
project README is kept here as the reference for individual features; the pages next to this one
(`software-features.md`, `hardware-controls.md`, `master-4-buttons.md`, ...) go deeper on each area.

## Current version

At launch, Mouse Studio opens on a connected supported mouse. You can select another mouse from the dropdown above the sidebar navigation. The MX Master 4 includes six official graphite photo views, and the M650 L includes Perspective and Top photos, with clickable button markers. Each mouse has independent profiles, saved settings and restore behavior; switching retains unsaved drafts. See [multiple mouse support](multiple-mice.md) for connection requirements and model-specific limitations. The Master 4 is verified through the C548 Bolt receiver: pointer controls, physical Back/Forward/Middle input, battery, sensor DPI/cycling, wheel modes and SmartShift. Its **wheel-mode button, gesture button and Haptic Sense panel button are now configurable**, including Click, Hold, Double-click, macros and special modes. Save installs the required background listener. See [Master 4 buttons](master-4-buttons.md). **Save also restores the side wheel to native horizontal scrolling**, with a separate Reverse side wheel control on Scrolling. This includes older Master 4 profiles and background readback/restoration after wake. See [side-wheel behavior](side-wheel.md). Extra-button/side-wheel physical verification, M650 L and Master Bluetooth verification are pending. The **Actions Ring** radial overlay is available as a button action (see [Actions Ring](actions-ring.md); its overlay window has not been visually verified). **Haptic feedback** (on/off, strength and a Test haptic button) and **side-wheel remapping** (shortcuts, volume, zoom, workspaces or commands instead of horizontal scrolling) are implemented for the MX Master 4 from published Solaar/logiops protocol notes. Both are **unverified on the physical mouse**; haptic controls stay hidden unless the mouse reports HID++ feature 0x19B0. See [side-wheel behavior](side-wheel.md) and [hardware controls](hardware-controls.md).

- Graphite and mint UI matched to the supplied concept, with Inter typography, consistent Lucide icons, a large labeled mouse image, and a dropdown-based inspector. Settings sits at the bottom of the sidebar.
- Responsive layouts down to 480 × 480: compact icon navigation, wrapping controls, scrollable dialogs, and a persistent Save bar in small or short windows. The 1536 × 1024 desktop design is preserved.
- Overview, Buttons, Pointer, Scrolling, Profiles, and Settings pages.
- Real read-only connected/disconnected status from Linux input devices.
- Product photographs plus a rotatable/zoomable photo-referenced 3D model, selectable hotspots and live press indicators.
- Working Back, Forward and middle-click assignments: applications, commands, keyboard shortcuts and workspaces.
- Optional automatic restore at login, mouse reconnect and Hyprland reload.
- Restore the previous saved setup, including profiles, button assignments and the automatic-restore preference.
- Pointer speed, acceleration, scroll speed, and direction controls.
- Live battery/charging status; native Bluetooth sensor DPI, named presets/cycling, wheel mode and SmartShift controls (verified on the connected Bluetooth mouse).
- Create, duplicate, rename, delete and reset profiles; manual or automatic switching by application class.
- Click, Hold and Double-click assignments, composed macros with delays, precision mode, horizontal-scroll combinations and directional gestures.
- Actions Ring: a configurable radial menu of up to eight actions at the cursor, with an editor preview.
- Save, discard, JSON import/export, validation, and unsaved-close confirmation.

**Four device controls now work:** pointer speed, pointer acceleration, scroll speed (0.1×–5×), and normal/reversed scrolling. Change them and press **Save profiles** (or **Ctrl+S**) to apply the selected profile to your MX Anywhere 3S in Hyprland. The app reports whether applying succeeded. No root access is needed.

Changes remain active after closing the app. In **Settings → Automatic restore**, enable the toggle and save to install the optional user service. It restores the saved active profile at login and within about five seconds of mouse reconnect or Hyprland reload. Disable the toggle and save to stop it when no profile needs horizontal scrolling or hardware controls. Those profiles also require this user service, which Save manages automatically. Otherwise, save again after a compositor restart/reload when Automatic restore is off. Opening the app or switching draft profiles alone does not apply changes. Failed applies preserve local profiles; Save retries.

Acceleration on selects adaptive movement; off selects a flat acceleration profile. Scroll speed 1× is normal; smaller values scroll less and larger values scroll farther.

**Button actions now work.** Choose the Back, Forward or middle button, select an action and save. Application and Command values use Hyprland’s command launcher; enter an installed executable or a command you intend to run. Shortcut examples: `CTRL+C`, `CTRL+SHIFT+C`, `ALT+Tab`, `SUPER+1`. Workspace values are 1–99. Default Click keeps normal button behavior; Default Hold/Double-click leaves that trigger unassigned. Suggested applications are not an installed-app inventory.

Saving/registering actions does not execute them. **Test action** runs the current draft action after three seconds; switch to the target window during that delay when testing a shortcut. Actual button actions target only `logitech-mx-anywhere-3s` on the verified Hyprland 0.56 Lua desktop. No root, logid changes or Hyprland config edits are needed.

The approved visual design is locked in [docs/ui-baseline.md](ui-baseline.md). See [device controls](device-controls.md) and [button actions and restore](buttons-and-restore.md) for behavior, limitations and verification.

Settings are saved explicitly to `$XDG_CONFIG_HOME/mouse-studio/settings.json`, or `~/.config/mouse-studio/settings.json`. Use **Save profiles** or **Ctrl+S**. Imports replace the current draft after confirmation; save to persist them. Discard restores the last saved state. Invalid input is rejected before replacing a saved file. An unreadable existing settings file blocks saving so it is not silently overwritten.

**Settings → Previous setup → Restore previous setup** undoes the last changed save after confirmation. It restores every saved profile and applies the selected one. The replaced saved setup becomes the new restore point; unchanged Save retries keep it. A restore point becomes available after saving a change to an existing setup. See [previous setup](previous-setup.md) for details.

The default window is 1536 × 1024, matching the supplied reference. On Overview and Buttons, save/discard controls sit in the inspector. The inspector moves below the main content in compact windows; scroll to reach it. Other pages retain the bottom save bar. On Hyprland, normal compositor tiling rules apply; float or maximize the window with your usual shortcuts if desired.

Click **Change** in the Application row to choose a suggested application or enter its command. The inspector supports Click, Hold and Double-click; **Advanced button settings** contains timing, precision, horizontal scrolling, gesture actions and a test pad. Choose **Macro sequence** in Action type to open the macro editor. Application matching and reset live on Profiles. See [software features](software-features.md) for instructions. See [hardware controls](hardware-controls.md) for sensor DPI, presets/cycling, wheel mode, SmartShift and battery. Battery reporting is live, and DPI/wheel/SmartShift writes and restoration were verified on the connected Bluetooth mouse.

## Assets

The Logitech product photographs and the Visual Studio Code icon are **not stored in this repository**;
`scripts/fetch-assets.sh` downloads them from their official sources (see [assets/README.md](../assets/README.md)).
Inter and Noto Sans Symbols 2 supply the typography under the SIL Open Font License; license files are in
`assets/fonts/`. Lucide SVG icons and their ISC license are in `assets/icons/`.

The editable mouse model is in [`assets/models/`](../assets/models/README.md), including Blender, GLB, four
rendered views and a [reference comparison](../assets/models/reference-comparison.html). Select **3D** in the app to rotate it.
