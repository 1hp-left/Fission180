# Connected sketch regions, Trim and Extend — 2026-10-19

Independent Fission 180 implementation after `0d0de6b`. No new Autodesk artifact/runtime investigation was performed. Fusion 2705.1.30 remains the selected reference; retained evidence in [rea-native.md](rea-native.md) is context, not evidence that these new interactions match Fusion.

## Implemented behavior

- A planar half-edge arrangement splits straight lines at finite intersections and coincident endpoints without replacing the source entities. Degree-three/four junctions, crossing dividers, shared boundary segments, corner-touching separate regions, exterior branches and bridges between loops are supported. Bridges cannot create a region. Nested disjoint loops still produce explicit holes.
- Each region retains a canonical cycle of source entity IDs. Coordinates and enumeration order do not name a region. Moving a divider preserves its two region IDs; adding a dangling branch preserves the existing region. Partition/merge consumers that lose their boundary return an error; the accepted document and solid remain unchanged. Retained inactive cycles allow construction toggles to restore their original IDs.
- Trim (ribbon, Modify menu, T) removes the clicked interval between intersections of finite straight lines. It can remove a whole line or leave two remnants. Extend (ribbon/Modify) extends the clicked end to the nearest finite line on that ray. No boundary, ambiguous trim-at-intersection and invalid inputs return errors.
- Replacement line entities receive new IDs. Affected driving dimensions, expression bindings and invalid constraints are removed through the entity-deletion dependency path. New cut/extended endpoints retain explicit point-on-line constraints on the boundary entities; a constrained boundary edit recomputes those endpoints. Unaffected geometry and constraints survive. Construction state propagates to replacement lines. Accepted edits use complete sketch Undo/Redo snapshots; failed commands retain the accepted draft.
- Sketch drawing includes complete source lines, including overhangs outside regions and geometry that no longer encloses a profile. Enabled tool states, crosshair, hover highlight, command hints and Escape use the existing native command routing.

Sketch arrangement, editing and solving execute independent Rust code. Solid extrusion executes the document's explicitly selected **OCCT** or **Rust analytic** backend. Both produce real solids from the discovered cells. OCCT remains the default; the Rust backend does not call OCCT or promote display triangles to solid geometry.

## Persistence and validation

New saves use **version 4**, still `openfusion-native` and millimetres. Versions 1–3 remain readable; v1 positional references are resolved before region migration. Version 4 persists retained boundary provenance and the point-on-line equation. Older executables do not support these additions. Open does not rewrite the source file. Invalid/duplicate/missing provenance and out-of-range region IDs are rejected. Touching/intersecting circles and coincident overlapping straight segments are rejected before an insertion is accepted.

## Executed evidence

`tests/arrangement.rs` verifies four 1500 mm³ crossing-divider cells on each backend, saved/reopened identities, OCCT STEP reimport, unchanged branch IDs, split/merge consumer rollback, moving-divider 2250/3750 mm³ results, construction restoration, nested holes, shared-vertex loops, bridges and malformed provenance. `tests/line_edit.rs` verifies trim remnants, dimensional cleanup, persisted dependencies, nearest/directional extension, boundary movement and rejected input. Desktop history tests verify complete Trim/Extend Undo/Redo snapshots.

`cargo fmt --all -- --check`, `cargo check --workspace`, and `cargo test --workspace -j 2` passed: **56 tests**. External logs are `arrangement-fmt.log`, `arrangement-check.log`, `arrangement-test.log` under `C:/Users/Vnm4/Downloads/openfusion-reference/development-2026-10-19`.

`cargo run -p fission180-core --example arrangement_corpus -- <external-directory>` saves/reopens five fixtures. `arrangement-fixtures/measurements.json` records four 5000 mm³ cells on each explicit backend and a 20000 mm³ OCCT STEP reimport, plus before/trimmed/extended sketch fixtures.

The separate native `fission180-arrangements.exe` was built with Cargo/rustc's explicit external output, preserving existing running executables. Fresh app-owned instances opened the saved Rust arrangement and trimmed fixtures, entered sketch editing, selected Extend through `--command Extend`, captured and exited themselves. Inspected `arrangement-sketch.png` and `arrangement-extend.png` are **1920×1080** at unchanged system scaling: four filled cells with complete divider overhangs; two retained trim remnants, persisted constraint glyphs, highlighted Extend command and its actual command hint. Binary/frame SHA-256 values are retained in `arrangement-identities.json`. This checks native rendering and reopened command state, not physical pointer/keyboard execution or matched Fusion parity. No Computer Use session resumed.

## Remaining limits

Curved arrangements, genuine arc entities/creation, Trim/Extend against curves, Offset, Project, sketch fillet, splines, self-touching boundary walks and overlapping collinear lines remain unsupported in this batch. Trim/Extend retire affected line dimensions and relations; general preservation/remapping of those design intents is not implemented. No exact Fusion snap, highlight, constraint glyph, command-state or visual match is established. Fresh Fusion/runtime comparison remains blocked after the prior physical-Escape stop; existing user windows were not controlled.
