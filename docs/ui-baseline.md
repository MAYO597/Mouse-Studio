# Approved UI baseline — 2026-10-07

The user approved the current Rust UI: “i like the ui let's lock it in”. Preserve this appearance while adding functionality.

- Reference: [approved Overview screenshot](baseline/overview.png). Other five pages are archived in `baseline/`.
- Keep the three-column layout, proportional sidebar/inspector and bottom-anchored Settings at desktop sizes, plus product photographs, hotspots and view selector.
- Responsive adaptation (2026-10-08): minimum window 480 × 480. Below 1100 pixels wide or 740 high, navigation becomes a tooltip-labeled icon rail. Below 1280 wide or 800 high, the inspector follows the page content and Save stays in the footer. Dialogs constrain their size to the viewport and scroll their bodies.
- Keep Inter typography, Lucide SVG icons, graphite surfaces, mint accents, spacing and control dimensions from `src/theme.rs` and `src/app.rs`.
- Keep the 1536 × 1024 default size and compact layout behavior.
- Use existing controls for functionality. Status text, capability descriptions and disabled states may change to reflect real behavior; this is not a redesign.
- Do not present unimplemented controls as functional or introduce fake device data.

The archived screenshots predate the first backend integration. Their preview/status wording is historical. Current screenshots can be generated with `cargo run --example capture`; this uses isolated settings and never applies device controls.
