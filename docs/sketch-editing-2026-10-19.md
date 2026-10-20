# Entity sketch editing — 2026-10-19

This follows the solid-command commit `08c3eca`. The independent sketch graph now participates directly in desktop editing rather than serving only primitive dimension dialogs.

## Implemented

- Canvas endpoint/center, edge and constraint-glyph selection; Ctrl/Shift adds items. Selections highlight their actual entities. Constraint glyphs reference actual stored equations.
- Working Horizontal/Vertical, Parallel, Perpendicular, Equal and Tangent ribbon actions. The Constraints menu also exposes Coincident, Concentric, Midpoint and Fix/UnFix. Invalid references and incompatible constraints return errors and preserve the accepted sketch.
- D edits a selected line length or circle radius using a persisted unit/named-parameter expression. Deleting a driving constraint removes its expression binding without deleting the profile. The existing rectangle width/height editor remains available for profile selection.
- Delete removes selected entities/points and their invalidated constraints/region bindings. Deleting one loop edge leaves the other edges open; Undo restores the complete previous graph. A downstream feature referencing a removed region rejects the model commit.
- X toggles construction state. Construction entities remain persisted and drawn dashed; affected loops cannot be consumed as solid regions until returned to normal geometry. Existing region identities survive construction toggles.
- Endpoint/center, midpoint and origin snapping before adaptive grid quantization, plus horizontal/vertical inference from the active line anchor. These are Fission policies, not measured Fusion snap thresholds.
- Point dragging adds a temporary solver target, preserves real constraints and records one accepted history step when the drag ends. Escape restores the pre-drag sketch. Fixed-point conflicts retain the last valid candidate.
- Simple degree-two closed line components become polygon regions backed by their original entity IDs, including reversed line directions. Editing a source sketch requests a disposable display rollback before its first dependent solid command; Finish Sketch replays the full document.

The solver uses central numerical derivatives. The new analytic regression exposed 0.002 mm³ of unintended volume drift in a 40→80 mm line-length edit with the earlier forward derivative; central derivatives remove that spurious motion and the 80×30×5 mm result meets the 1e-4 mm³ acceptance on both backends.

## Evidence and limits

`tests/sketch_edit.rs` covers reversed chain closure, stable region IDs, named length changes and 12,000 mm³ extrusion on explicit OCCT/Rust backends, file Save/reopen, construction restoration, edge deletion/reclosure, downstream rollback, dimensional-constraint deletion, dragging conflicts, midpoint/concentric/equal-circle solving and missing-reference rejection. Desktop snap regression checks candidate priority and axis inference. Existing complete sketch-history snapshots carry these edits through Undo/Redo.

`sketch_corpus` generates saved/reopened entity-edit fixtures with a dimensioned line loop, construction segment and circle parameter on both backends. External evidence lives under `C:/Users/Vnm4/Downloads/openfusion-reference/development-2026-10-19/sketch-fixtures`.

The complete workspace run passes **44 tests**, including all prior worker/integrity and solid-command checks. Exact commands `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo test --workspace -j 2` passed; external logs are `sketch-fmt.log`, `sketch-check.log`, `sketch-test.log`. A native build at external `fission180-sketch.exe` was operated using `--open <entity-edit-rust.ofrust> --sketch --capture <path.png> --exit-after-capture`. The initial `sketch-entity-edit.png` at 1920×1080 was inspected: source-sketch rollback, line loop, circle, construction dash and constraint glyphs were visible. That inspection prompted numeric dimension labels and proper concave polygon fill rather than leaving the line region unfilled. Final capture is `sketch-entity-edit-final.png`. App-owned captures do not establish mouse/key routing parity; Computer Use has not resumed.

General intersecting/branched line arrangements, arcs, splines, trimming, sketch fillet/offset/project, driven/angle dimensions, arbitrary persistent face attachment, box selection and a complete Fusion constraint diagnosis UI remain unsupported. Region fill and glyph placement still differ from Fusion. Solver limits remain 128 points and 256 constraints. No new Autodesk artifact/runtime investigation was performed for these source changes; retained REA evidence supplies investigation context only. Full runtime, camera, command and visual parity remain unestablished.
