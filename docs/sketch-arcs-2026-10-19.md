# Analytic sketch arcs — 2026-10-19

Independent implementation following `f41a008`. This extends the Rust sketch engine; production arc-bounded solid regions are not implemented in this batch.

## Implemented

- Genuine bounded circular entities store stable center, start and end point dependencies and signed branch direction. The through point selects the circle and branch during creation; it is not persisted as a fictitious polygon vertex. Analytic radius, sweep, length, segment area, point evaluation, nearest-distance and inclusion queries are independent Rust mathematics. View sampling is display-only.
- Three-point creation is available through Sketch Create → 3-Point Arc and A. Click the two endpoints, then a point on the arc. The native viewport previews the chord, then the arc and radius. Invalid/collinear cursor input retains the last valid arc preview; failed acceptance retains the previous graph and pending endpoints. Enter finishes the tool; Escape discards pending input. Repeated creation is supported.
- Endpoint/center and true arc-midpoint snapping; analytic finite-arc picking; selection highlight; radius editing with D using persisted expressions/parameters; constrained endpoint/center dragging; construction display; Fix/UnFix, equal radius, concentric and line/arc tangency; entity/point deletion and complete sketch Undo/Redo.
- Each arc always contributes its equal-endpoint-radius equation to the solver, including construction arcs. Finite line/arc tangency is checked after solving: contact on the supporting circle outside the actual arc or line is rejected. Degenerate arcs, missing dependencies, invalid persisted solutions and solver conflicts return errors. The accepted graph remains unchanged.
- A normal arc touching/inside a cached straight/circular profile conservatively makes that profile unavailable for solids. The profile is unfilled and an attempted consumption reports unsupported curved-region geometry. Construction arcs do not block the existing region. This deliberately avoids extruding an old straight-sided profile while ignoring a new curved subdivision. Even a dangling internal arc currently disables its containing region; exact curved arrangement classification remains work to do.

The published [Autodesk arc creation guide](https://help.autodesk.com/cloudhelp/ENU/Fusion-Sketch/files/SKT-CREATE-ARCS.htm) specifies endpoint → endpoint → point-on-arc creation and Enter completion. It informed that command order. This is current public documentation, **not** new runtime evidence for build 2705.1.30. A is a Fission shortcut in this implementation; exact selected-build routing and automatic inferred constraints remain unverified. No native Autodesk code/assets were copied, no target artifact investigated and no REA Evidence ID fabricated.

## Persistence, backend and acceptance

Version **4**, `openfusion-native`, millimetres, now includes the arc entity variant. Versions 1–3 remain readable; Open never rewrites those files. Geometry and constraint replay occur in the independent Rust sketch engine regardless of the persisted solid backend. OCCT remains the default solid kernel. No Rust-to-OCCT fallback was added.

`tests/arcs.rs` covers clockwise/minor/major branches, finite analytic picking, degenerate input, named radius replay/save/reopen, both endpoint radii, equal/concentric relations, finite tangency, conflicts/retention, shared line endpoints, construction/deletion and unavailable curved cells. A test-only adapter reconstructs an analytic semicircular segment as OCCT edges, checks its prism volume within 1e-5 mm³ and STEP reimport. That adapter is interoperability evidence, not production curved-region extrusion or a Rust solid-kernel replacement.

Desktop tests cover retained last-valid arc preview, actual arc-midpoint snapping and complete creation/radius/construction Undo/Redo snapshots. The existing transactional worker, expressions, arrangements, extents and solid-command tests remain active.

`cargo fmt --all -- --check`, `cargo check --workspace`, and `cargo test --workspace -j 2` passed: **64 tests**. Logs: external `arcs-fmt.log`, `arcs-check.log`, `arcs-test.log` under `C:/Users/Vnm4/Downloads/openfusion-reference/development-2026-10-19`. `arc_corpus` saved/reopened two native fixtures with three arcs each; `arc-fixtures/measurements.json` records exact radius/sweep/length and construction state plus the executing Rust sketch engine separately from the chosen solid backend.

The separately built external `fission180-arcs.exe` opened `arcs-rust.ofrust`, entered sketch editing and selected `--command '3-Point Arc'` in a fresh app-owned instance, then captured and exited itself. Inspected `arcs-sketch.png` is **1920×1080** at unchanged system DPI: the 40 mm upper semicircle, actual tangent line, R 40.00 label, dashed construction semicircle, separate quarter arc, and live first-endpoint command hint are visible. Hashes are retained in `arcs-identities.json`. This verifies native rendering, persisted replay and command entry; physical three-click/keyboard execution, palette Feature Options and matched Fusion screenshots remain unverified. Computer Use was not resumed.

## Remaining scope

Curved region construction and extrusion, arc/curve Trim/Extend, point-on-arc constraints, arc/arc tangency, angular/driven dimensions, Center Point/Tangent Arc creation variants, Offset and Project remain unsupported. The solver retains its 128-point/256-constraint limit. Automatic snapping constraints, full numerical entry during creation, glyph placement, camera, menus and physical keyboard/pointer behavior still require matched Fusion scenarios. Existing user windows were not controlled; fresh Fusion comparison remains blocked after the physical-Escape stop.
