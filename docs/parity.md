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
| Plane selection and sketch entry/exit | XY/XZ/YZ origin planes | Runtime XY scenario | Other planes, existing faces, orientation, cancellation, selection filters |
| Two-point rectangle and center/radius circle | Working profiles | Rectangle runtime; command declarations | Live dimensions, snaps, construction state, constraints, chained creation |
| Lines, arcs, splines, polygons, slots, text | Closed polygon foundation | Shipped command inventory | Full entity model and tool interactions |
| Dimensions and expressions | Persisted unit expressions, named parameters, graph dimensions and sketch Undo/Redo | Shipped QML, native adornment and deletion dossiers, one accepted unit expression | Full active editor, expression grammar, parameter completion, driven dimensions, delete/undo semantics |
| Constraint solver | Initial entity graph, damped solver, DOF/conflict reporting | Shipped commands; native solving-request wrapper | Solver algorithm, degrees of freedom, tolerances, overconstraint diagnosis, glyphs, editing |
| Sketch grid, highlights, camera | Approximate grid and cached projected display | Four native sketch UI dossiers | Runtime correlation, grid interval logic, selection states, depth and occlusion |
| Extrusion | OCCT and explicitly selected Rust analytic extrusion; regions with holes, multiple/empty outputs, targeting and timeline edit | Runtime dialog and one measured 40 × 20 × 6 mm body; bounded assembly trace; full Ghidra import timed out | Native body-creation dossier, selection rules, extent/start modes, taper, multiple profiles/bodies, failures, expression entry |
| Revolve, sweep, loft, hole, patterns, fillet, chamfer, shell | Unimplemented | Partial shipped toolbar inventory | Native entry traces, controlled scenarios, independent modeling operations |
| History and dependencies | Transactional worker, isolated histories, stable profiles/regions/body outputs, mesh failure rollback | Fission 180 tests | Fusion-side edit/suppress/failure comparison, stable topology references |
| Files and interchange | Atomic queued Save, failed Open retention, v1 migration/v2 saves; OCCT STEP/STL; Rust exchange test adapter | Fission 180 analytic and STEP round-trip tests | Fusion-side neutral-format comparison, native format investigation |
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
