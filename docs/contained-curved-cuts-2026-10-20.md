# Contained partial-depth curved cuts — 2026-10-20

Independent Rust engine batch after curved-extrusion commit `110b261`. OCCT remains the default. No Fusion runtime or existing user window was controlled.

## Implemented scope

The Rust backend now subtracts a strictly contained line/circular-arc profile through one extrusion end cap, leaving a planar floor and exact inward-facing trimmed cylindrical/planar walls. Both end caps, positive/negative target extrusion, reversed tool extrusion, XY/XZ/YZ and translated tool frames are supported. Tools with circular, semicircular, capsule and other valid line/arc boundaries use the same construction. Curved targets and existing disjoint through-holes are supported. This is a bounded analytic construction, not a general surface-intersection Boolean algorithm.

Successive pockets are accepted when their planar footprints are strictly disjoint from existing holes and pockets. The limit is 64 pockets. A later disjoint through-cut preserves existing pockets; translation and copies retain their geometry. Fully covered targets produce zero bodies, accepted contained cuts produce one, and the earlier disjoint-Join and box-splitting multiple-body paths remain available. Pocketed bodies cannot be mistaken for their rectangular envelopes by the box Boolean or identical-solid shortcuts.

The boundary representation adds each pocket opening as a real cap inner loop, reverses the removed prism's retained walls/floor, and shares those exact edges with the opening. It validates the resulting connected manifold shell before acceptance. Cylindrical UV trim orientation is now checked analytically as well as planar orientation. Provenance retains both parent histories, records the actual Cut, and excludes self-referential transient tool parents.

Volume subtracts exact profile area times penetration depth. Surface area adds exact wall perimeter times depth; the removed opening and added floor have equal area. Bounds retain the strictly containing target's extrema. Display cap openings and floors are tessellated separately; sampled triangles never become the solid. Display edges now use the correct source-span ordinal at arc/line junctions. The additional disjoint-void regression found and repaired floating-point endpoint ownership in analytic winding (for example, `sin(π)` at a diameter join).

All computation in `kernel/native/cuts.rs` and the native geometry path is independent Rust. The OCCT path is used only when selected explicitly or by the labeled test oracle. Rust neutral-format export remains unsupported.

## Replay and acceptance

Existing version-5 extrusion/Hole/Combine feature data reconstructs these solids during replay; no serialized kernel handles or additional schema are needed. Versions 1–4 retain their migration path. Named depth expressions, accepted state, complete document Undo/Redo, save/reopen and invalid dependency retention use the ordinary session transaction. The existing Hole command can now make a supported contained blind circular cut when Rust is selected.

`tests/curved_cuts.rs` covers 72 circle/semicircle/capsule combinations of principal plane, target direction, entering cap and tool direction, including translated coordinate frames. An explicit test oracle constructs/cuts exact curves independently with OCCT, compares analytic volume/area, and checks STEP reimport. Additional cases cover curved targets with holes, multiple disjoint pockets, through-cut preservation, translations, body-scoped topology references, oriented display-shell volume, tessellation-independent mass properties, history and unsupported edits. The earlier curved tests additionally verify a consuming child region rejects a merge.

`curved_cut_corpus` saves/reopens 18 backend-tagged blind-cut documents under `C:/Users/Vnm4/Downloads/openfusion-reference/engine-2026-10-20/curved-cut-fixtures`. Each measurement record identifies its executing backend, expected/measured volume and surface area, bounds, and Rust manifold topology. OCCT fixtures retain STEP files and reimport volumes. These are independent implementation checks, not Fusion interchange or visual parity evidence.

## Explicit unsupported cases

Side-wall crossings or tangencies, cuts touching/intersecting existing voids, overlapping pocket footprints (including opposite-side overlaps), enclosed cavities, annular cutting tools, nonparallel-axis curved cuts and arbitrary curved Join/Intersect remain unsupported. These return errors and retain the accepted document/geometry. Near-boundary geometry uses the existing millimetre tolerance policy; no general healing or robust-predicate kernel is claimed. Full-circle crossing arrangements, curve Trim/Extend, splines, general topology naming and full Fusion parity remain outside this batch.

Executed `cargo fmt --all -- --check`, `cargo check --workspace` and `cargo test --workspace -j 2`: all passed, **75 tests**. External logs: `cuts-fmt.log`, `cuts-check.log`, `cuts-test.log`, `cuts-corpus.log`; `cuts-results.json` records the results. Fixture volume error was at most 7.3e-12 mm³, area error 1.9e-12 mm², and STEP reimport volume difference 2.2e-11 mm³. The final tests also check stable pocket edge identities during depth edits, named hole-diameter replay and rollback, and exhausted topology-ID rejection.
