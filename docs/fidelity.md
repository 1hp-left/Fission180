# Fusion interface reconstruction

## Reference

Observed running application: Autodesk Fusion, file/product version 2705.1.30. Current preference: Match Device Theme, visibly dark blue. Windows display scaling 125%, display resolution 1920 × 1080. Maximized window observed as a 1536 × 864 logical-pixel screenshot; the Rust renderer produces a 1920 × 1080 physical-pixel framebuffer at that scaling.

The independent application is now Fission 180 (formerly OpenFusion). The [2026-10-19 audit](audit-2026-10-19.md) remains authoritative for known defects and comparison coverage. A fresh reference session recovered clean Home, XY sketch, rectangle and finished-sketch frames; see [the implementation record](development-2026-10-19.md). Desktop automation was then stopped before complete matched solid/Fission comparisons, so those remain unverified. The rectangle Feature Options state is now represented; exact glyphs/typography and disabled variants remain discrepancies.

Reference screenshots are retained outside the source tree at `C:\Users\Vnm4\Downloads\openfusion-reference\2705.1.30`. No extracted Autodesk icons, bundled UI resources, or account data are included in the application.

## Measured layout

| Region | Logical coordinates / size |
| --- | --- |
| Custom title bar | y = 0–24 |
| Quick access and document tab | y = 24–55 |
| Workspace tabs and ribbon | y = 55–140 |
| Design selector | x = 11, y = 65, 110 × 61 |
| Browser header | x = 4, y = 143, 299 × 25 |
| Main viewport | y = 140 through window height − 50 |
| View origin, initial view | center of the viewport |
| Sketch Palette | right edge − 254, y = 258, width = 250 |
| Extrude panel | right edge − 254, y = 258, 250 × 314 |
| Comments bar | x = 4, bottom − 80, width = 299 |
| Navigation strip | horizontally centered, bottom − 82 |
| Timeline | last 50 logical pixels |

The implementation follows system DPI. It reads Segoe UI and Segoe Symbols from the Windows installation at runtime, with bundled egui fonts as fallback. It does not redistribute Microsoft's font files.

## Observed Fusion behavior

- A new Part Design opens the Design workspace with the Solid ribbon and an isometric grid.
- Create Sketch displays the three origin planes for selection.
- Selecting XY changes to an orthographic Top view and adds the Sketch ribbon and right-hand Sketch Palette.
- R activates two-corner rectangle creation. The observed 40 × 20 mm rectangle had an 800 mm² profile area.
- Finish Sketch returns to the Solid ribbon and isometric view.
- E opens a right-hand Extrude panel with type, profiles, start, direction, extent, distance, taper angle, and operation controls.

A later scratch-design [input scenario](runtime-extrude-input.md) retained Top view after Finish Sketch. Camera reorientation depends on more state than the initial observation established. That pass also observed separate invalid HUD draft and last-valid palette values, followed by command cancellation on Escape.

These are Computer Use observations. They do not establish Fusion's internal source code or algorithms.

## Verified implementation behavior

The native application was manually exercised through Create Sketch → XY → R → two corners → E → extrusion preview → OK. Native unit tests separately verify the resulting geometry, Boolean operations, history, rollback, and neutral-format round trips.

## Known visual and interaction differences

- Toolbar icons are original approximations. Detailed silhouettes, shading, and some control glyphs still differ from Fusion.
- Typography and antialiasing use egui rather than Qt; no exact whole-screen pixel-error threshold has been achieved or asserted.
- View-cube labels and interactions are simplified. Grid projection, labels, and contrast still need broader camera-state comparison.
- Hover tooltips, marking/context menus, snap glyphs, constraint badges, live dimension entry, dock resizing, and full keyboard navigation are incomplete.
- Entity selection, constraint actions/glyphs, constrained dragging, construction display and endpoint/midpoint/axis snapping now have implementations; their exact Fusion placement, thresholds, command states and rendering remain unmatched. See [entity editing evidence](sketch-editing-2026-10-19.md).
- Trim/Extend now have enabled native ribbon/menu controls, command hints, source-line hover and sketch history. Connected line cells are selectable profiles. Their Fusion interaction/visual equivalence remains unverified; see [arrangement evidence](sketch-arrangements-2026-10-19.md).
- The active tool's extra Feature Options section in Fusion's Sketch Palette is not fully reproduced.
- Several ribbon areas are scaffolding. The Home screen, other workspaces, and account/cloud experiences are not faithful implementations yet.
- The initial viewport uses cached projected triangles through egui/wgpu. A dedicated depth-buffered CAD render pass and robust face/edge picking remain future work.

The current milestone is a native desktop foundation based on measured Fusion states. It should not be described as a pixel-perfect or feature-complete Fusion replacement.
