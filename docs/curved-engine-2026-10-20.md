# Exact curved regions and Rust extrusion — 2026-10-20

Independent Fission 180 implementation following `5f05311`. This engine phase uses ordinary source tools. No Fusion runtime investigation or desktop automation was resumed; no Autodesk algorithms or assets were adopted. OCCT remains the default backend.

## Implemented geometry

`boundary.rs` retains oriented line segments and bounded circular arcs, including center, radius, start angle, signed sweep and optional original entity ID. Area uses chord and circular-segment integrals; perimeter, cardinal extrema, containment and finite intersections operate on the analytic curves. No sampled polygon is used as authoritative geometry.

The sketch arrangement now splits virtual spans at line/arc and arc/arc intersections, joins shared endpoints, removes bridges, orders outgoing analytic tangents and discovers oriented bounded faces. Consecutive spans on the same source are compacted after bridge removal. Nested loops produce immediate holes. Complementary co-circular arcs and ordinary degree-two tangent joins are supported; overlapping curves and multi-branch tangent junctions are rejected explicitly. Full-circle primitive crossings with the arrangement remain outside this batch.

Line-only region IDs retain their prior source-cycle semantics. Curved regions retain directed entity/junction keys and finite intersection branch ordinals. Coordinates and display indices do not name a region. Dimensional edits retain IDs while an unchanged boundary exists. Splits, merges and deleted dependencies cannot redirect a consuming extrusion. Accepted extrusions additionally snapshot all outer/hole IDs; changing that selection requires explicit repair. This is scoped source provenance, not general persistent naming across arbitrary topology changes.

The Rust kernel builds true trimmed circular edges, cylindrical side faces and planar caps with shared vertices/edges and oriented coedges. Positive/negative extrusion, XY/XZ/YZ, translated origins, concavity and disjoint holes are supported. Edge/face records expose original sketch entity provenance where available. Validation checks finite geometry, exact arc endpoint parameters, curve/surface incidence, closed loops, opposed edge uses, planar face orientation, connected face shells and circular vertex links. The additional orientation check exposed and repaired a pre-existing negative-extrusion side-face reversal.

Volume, area and bounds remain analytic at every display resolution. Display tessellation carries the correct analytic source-face IDs rather than assigning a new face to each sampled arc chord. Rust execution contains no OCCT modeling, conversion or fallback call. OCCT production extrusion can independently consume the same exact curved profile edges.

## Persistence and transactions

New saves use **version 5**, `openfusion-native`, millimetres. Versions 1–4 remain readable through in-memory migration. Old positional references are resolved before region migration; new curved caches are regenerated from old arc graphs. Unsupported/ambiguous former selections fail explicitly. Open never rewrites the input file. A lower version cannot encode the new curved semantics. Document validation compares cached geometry with derived analytic regions and checks retained provenance.

Radius expressions and named parameters recompute the graph and feature replay. Complete document snapshots provide Undo/Redo. Failed graph changes, missing boundaries and changed hole selections retain the accepted document and solid. Desktop changes are limited to compatibility with the new extrusion dependency field; no UI styling or dialogs were added.

## Acceptance and remaining scope

`tests/curved.rs` checks the radius-10 semicircle at depth 5 (**250π mm³**), quarter sectors, capsules, major arcs, complementary arcs, curved holes, all principal planes, both distances, reversed boundaries and translated geometry. It checks exact bounds/area/volume, source-face tessellation IDs, stable dimensional/topology identities, radius expressions, saved/reopened files, Undo/Redo, split/missing/hole dependency rollback and malformed topology. It independently reconstructs OCCT edges from the Rust B-rep, compares measurements, and reimports STEP. That adapter is test-only and does not implement Rust export.

`curved_corpus` writes/reopens 18 backend-tagged fixtures with measurements outside the repository, under `C:/Users/Vnm4/Downloads/openfusion-reference/engine-2026-10-20/curved-fixtures`. Rust and OCCT results are separately identified. The OCCT fixtures also retain STEP exports/reimport measurements.

General Boolean intersections, full-circle crossing arrangements, coincident overlaps, multi-branch tangencies, splines, arbitrary surface trimming/healing and production Rust STEP export remain unsupported. Sketch solving retains its existing 128-point/256-constraint limit; arrangement discovery additionally bounds its source/intersection/traversal sizes. UI/Fusion parity and kernel replacement are not established by these tests.

Executed `cargo fmt --all -- --check`, `cargo check --workspace` and `cargo test --workspace -j 2`: all passed, **70 tests**. External logs: `curved-fmt.log`, `curved-check.log`, `curved-test.log`, `curved-corpus.log`; `curved-results.json` records exit codes and fixture count. Maximum fixture volume error was 2.3e-13 mm³ against the analytic expectations.
