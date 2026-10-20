# Fusion parity ledger

Reference: installed Fusion 2705.1.30, Match Device Theme with dark-blue appearance, 1920 × 1080, Windows at 125% scaling. The goal is an independent Rust implementation with matching visible behavior across the product. Sketching and solid modeling are the first working slice. No subsystem currently has verified complete parity.

The [2026-10-19 audit](audit-2026-10-19.md) records the reproduced failed-Open/Save overwrite and sketch undo/redo defects, unsupported unit expressions, and topology/constraint/workspace gaps. Several defects now have source repairs and regression tests; see [the implementation/evidence record](development-2026-10-19.md). They are not promoted to Fusion workflow matches. Fresh reference Home/sketch captures were recovered, but desktop automation was stopped before matched solid and Fission retests completed.

## Acceptance evidence

Each command needs a reproducible scenario with its initial document, selection, inputs, actions, intermediate states, result and undo/redo sequence. Compare screenshots at matching logical size, DPI, theme and camera. Record geometric measurements and interchange results independently of screenshots. Error paths, cancellation, keyboard input and persistence are part of the scenario.

Use analytic tolerances appropriate to each operation and unit; record them with the test. Record visual differences by region and state. A low average image error does not prove that a missing control or incorrect interaction is acceptable. Performance comparisons need the same model, action and machine, including cold and warm measurements.

## Coverage

| Area | Independent Fission 180 implementation | Fusion evidence | Remaining verification |
| --- | --- | --- | --- |
| Window, ribbon, browser, timeline, palettes | Native Rust foundation | Measured screenshots; shipped toolbar XML | Exact icons, typography, docking, menus, accessibility, command availability |
| Plane selection and sketch entry/exit | XY/XZ/YZ origin planes and principal extreme planar body supports, recomputed after upstream edits | Runtime XY scenario | Arbitrary faces, exact orientation, cancellation, selection filters |
| Two-point rectangle and center/radius circle | Working profiles | Rectangle runtime; command declarations | Live dimensions, snaps, construction state, constraints, chained creation |
| Lines, arcs, splines, polygons, slots, text | Connected line arrangements/stable cell IDs, line Trim/Extend, genuine arcs, exact line/arc and arc/arc region discovery, native three-point creation, construction/deletion; [line evidence](sketch-arrangements-2026-10-19.md), [arc evidence](sketch-arcs-2026-10-19.md) | Shipped inventory; published arc click-order guide | Full-circle crossing arrangements, curve Trim/Extend, Offset/Project, splines, slots, text and matched tool interactions |
| Dimensions and expressions | Persisted unit expressions, named parameters, graph dimensions and sketch Undo/Redo | Shipped QML, native adornment and deletion dossiers, one accepted unit expression | Full active editor, expression grammar, parameter completion, driven dimensions, delete/undo semantics |
| Constraint solver | Entity selection, geometric constraints, line/circle/arc dimensions, finite line/arc tangency, construction, constrained dragging, damped solver and DOF reporting | Shipped commands; native solving-request wrapper | Matched glyphs/tolerances/diagnosis, point-on-arc and arc/arc tangency, angle/driven dimensions and larger sketches |
| Sketch grid, highlights, camera | Approximate grid and cached projected display | Four native sketch UI dossiers | Runtime correlation, grid interval logic, selection states, depth and occlusion |
| Extrusion | OCCT and explicit Rust analytic line/arc extrusion; curved regions/holes, target selection, timeline edits, offset start, two-sided/symmetric and through-all extents | Runtime dialog and one measured 40 × 20 × 6 mm body; bounded assembly trace; full Ghidra import timed out | Native body-creation dossier, exact selection/state behavior, To Object, taper/thin extrusion, multiple profiles/bodies and matched extent scenarios |
| Revolve, sweep, loft, hole, patterns, fillet, chamfer, shell | Real replayable solids with scoped selectors and editable dialogs; [command limits](features-2026-10-19.md) | Partial shipped toolbar inventory | Full selectors/options, native entry traces, matched runtime command scenarios and screenshots |
| Transform, Combine, Split Body, Measure | Translation/copy, mirror, rectangular/circular patterns, exact Boolean and principal-plane split, volume/area/bounds | Independent analytic and STEP checks | Fusion command states, arbitrary axes/planes, selection and measurement annotations |
| History and dependencies | Transactional worker, isolated histories, stable output IDs, mixed timeline, dependent suppression/deletion and face supports | Fission 180 regression tests | Fusion-side edit/suppress/failure comparison, persistent arbitrary topology references |
| Files and interchange | Atomic queued Save, failed Open retention, v1–4 migration/v5 saves with analytic curved region provenance and extrusion boundary dependencies; OCCT STEP/STL; Rust exchange test adapter | Saved/reopened command/arrangement fixtures and OCCT STEP measurements | Fusion-side neutral-format comparison, native format investigation |
| Assemblies, joints, configurations | Unimplemented | Uninvestigated | Inventory, behavior contracts, implementation and verification |
| Surface, mesh, sheet metal, form, electronics | Unimplemented | Uninvestigated | Workspace-specific investigations |
| Drawings, rendering, animation, simulation, manufacturing | Unimplemented | Uninvestigated | Workspace-specific investigations |
| Home, data, collaboration, cloud services, extensions | Simplified local Home | Initial Home observation only | Service contracts, availability, offline behavior, independent services |
| Large models and efficiency | Geometry worker, cached display, small-part benchmark | Fission 180-only benchmark | Matched workloads, memory, startup, idle CPU, interaction latency |

## First comparison scenarios

1. Create a 40 × 30 mm rectangle on XY, extrude 5 mm, edit width to 80 mm, add a 10 mm diameter through-hole, undo and redo each edit. Compare solid count, dimensions, volume, history and saved/reopened results.
2. During an active command, type valid and invalid dimensional expressions, use Escape and Enter, change focus and cancel. Record draft text, error presentation, preview lifetime and the final document separately.
3. Exercise each extrusion operation against overlapping, tangent and disjoint solids, with positive and negative distances and multiple selected profiles. Preserve the last valid document on failure in Fission 180.
4. Repeat sketch selection, hover, camera orbit/zoom and grid toggles at matching DPI. Compare geometry display separately from toolbar and palette regions.

These are pending comparison protocols, not completed tests. Link each completed scenario and its evidence in [investigation.md](investigation.md). Native analysis details are in [rea-native.md](rea-native.md).

## Independent engine phase — 2026-10-20

[Exact curved regions and extrusion](curved-engine-2026-10-20.md) records the Rust implementation and its analytic/interoperability acceptance. No fresh Fusion runtime or UI comparison was attempted. General curved Booleans and kernel replacement remain incomplete; the default backend is still OCCT.
