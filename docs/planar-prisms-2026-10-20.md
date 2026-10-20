# Analytic planar and uniform-prism Booleans — 2026-10-20

Independent mathematical implementation after `d2b2747`. No Fusion investigation,
decompiled material, external assets, or desktop automation was used. OCCT remains
installed and selected by default. These are implementation checks, not Fusion
parity evidence or production Rust neutral-format export.

## Geometry and topology

`planar.rs` implements regularized Union, Difference and Intersection of bounded
line/circular-arc regions. Circles enter as two analytic half arcs. The engine
intersects finite supports, splits at crossings and coincident interval endpoints,
classifies material on both sides of each bounded span, removes duplicate and
internal boundaries, and assembles directed loops. Positive loops own their
immediate negative holes; islands in holes become separate regions. Empty and
multiple-region results are explicit. Equivalent circle segmentation, reversed
input boundaries, ordinary shared line spans and co-circular intervals are
supported. No sampled polygon or mesh decides the Boolean.

`kernel/native/prisms.rs` proves compatible principal-plane frames and occupied
axial intervals from extrusion parameters and actual analytic region geometry.
Translated origins and opposite extrusion directions normalize into the target
frame. Cut also accepts an axial tool interval covering the entire target.
Pocketed solids are excluded from this uniform-prism path. Existing contained
cuts and scoped box operations remain available through their existing paths.

Each connected region becomes a validated shared-edge analytic B-rep with planar
caps, planar or cylindrical sides, and opposed coedges. There are no interfaces
between overlapping operands. Volume is area times axial length; surface area is
twice the area plus total boundary perimeter times axial length. Bounds include
exact arc extrema. Tessellation references the real retained faces and edges and
does not affect measurements. Translation and pattern instances preserve source
ancestry with semantic instance identifiers.

## Numerical policy and limits

This is floating-point analytic geometry, not symbolic or arbitrary precision.
Metric coincidence uses 0.0000001 mm; the minimum resolved separation/span is
0.00001 mm, consistent with existing profile validation. Angular parallelism uses
1e-10 radians. The existing ±1,000,000 mm finite domain remains in force. Near
coincident/tangent supports between the two metric thresholds fail explicitly;
the engine does not grow tolerances or heal unrelated geometry. Exact tangencies
are accepted when the retained result is regular: an external tangent disk
Intersection is empty, while a point-contact Union is rejected as nonmanifold.

Limits are 512 combined input curves, 32,768 intersection events, 8,192 split
spans and bounded loop traversal. Tiny splits, ambiguous vertex clusters,
disconnected/self-intersecting loops, nested/overlapping operand holes, invalid
source intervals and nonfinite data fail before publication. Positive-area
regions touching only at a point are deliberately rejected. Resource limits and
conservative tolerance rejection bound this implementation's scope.

## Source identity and application contracts

Every span retains its original feature, region, boundary, entity (where present),
original primitive edge ordinal, transform instance path and directed source
parameter interval. Reversal reverses ancestry intervals; coincident retained
spans carry both ancestries. Endpoint and intersection relationship keys survive
assembly. Body ordering and Boolean edge/face identities derive from these
relationships, without sampled coordinates or result-array positions. Digest
collisions fail explicitly. This remains scoped source-based naming; arbitrary
topology edits and general persistent naming are not solved.

Existing extrusion operations and Combine replay use this engine in the native
application. Split output IDs are materialized and saved; changed Boolean counts
or accepted source relationships require explicit repair instead of redirecting
dependencies. Pattern quantity editing retains its existing independent behavior.
Failed rebuilds and display generation preserve accepted documents and solids.
Version-5 semantics suffice; no persisted kernel handles or schema changes were
introduced. Versions 1–4 retain their existing migration path; opening does not
write the file. Rust performs no OCCT modeling, conversion or fallback.

## Acceptance and evidence

Reusable input generators live in `examples/support/prism_fixtures.rs`, with
`tests/planar_booleans.rs`, `tests/prism_booleans.rs` and `examples/prism_corpus.rs`.
The fixtures include the radius-10/separation-10 analytic lens, 40×20 half-disk
notch, strip split, concentric annular cut, disjoint union and full removal.
Planar tests additionally cover coincident segmentation, nesting, metamorphic
identities, malformed provenance, near tangencies and resource bounds. Solid
tests exercise all principal planes, translated frames, negative and opposite
extrusion directions, exact analytic bounds, manifold topology, display
resolution independence, expressions, replay, save/reopen, history and rollback.

The explicitly labeled OCCT oracle constructs its operands independently from
the same input specification and compares body counts, volume and surface area.
OCCT STEP export/reimport validates only that oracle/interoperability path.

External evidence directory:
`C:/Users/Vnm4/Downloads/openfusion-reference/planar-prisms-2026-10-20`.
Final commands, outcomes and fixture measurements are recorded there.

Executed `cargo fmt --all -- --check`, `cargo check --workspace` and
`cargo test --workspace -j 2`: all passed, **84 tests** (baseline 75).
`cargo run -p fission180-core --example prism_corpus -j 2 -- <external fixtures>`
saved/reopened **32** backend-tagged extrusion/Combine fixtures. Logs and summary
JSON include the final outcomes. Earlier failed fixture/integration runs are
retained separately. The initial baseline's 75 tests passed, but its rustdoc
phase raced the first module addition and failed; the final complete run,
including rustdoc, passed.

General nonparallel/unequal-interval curved Booleans, blind sidewall crossings,
opposite-side overlapping pockets, arbitrary surface intersections, full-circle
crossings in sketch arrangement discovery, splines, production Rust STEP export
and complete Fusion parity remain outside this batch. No distribution package
or new dependency was introduced; this batch required no new research or
distribution clearance decision.
