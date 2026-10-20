# Overlapping same-cap blind pockets — 2026-10-20

Independent mathematical implementation following planar/prism commit `7267ce9`.
The planar/prism batch passed all required workspace checks and was committed
and pushed before this batch began. No Fusion runtime, retained decompilation,
desktop automation, new dependencies or distributed artifacts were involved.
OCCT remains the default production backend.

## Occupied volume and boundary construction

The Rust backend accepts overlapping strictly contained blind pockets entering
the same extrusion cap. Exact line/circular-arc footprint regions, including
holes, are subtracted from the base cross-section over each pocket's occupied
axial interval. Pocket floor events partition the extrusion into slabs; every
slab stores its actual occupied analytic regions. Removal is counted once even
where footprints overlap and their depths differ.

`kernel/native/steps.rs` constructs side patches from each slab's occupied
boundary. At each floor it constructs only the regularized difference between
adjacent occupied cross-sections. Internal interfaces are omitted. All patch
curves are refined to shared analytic junctions, then vertices and exact bounded
edges are shared across incident faces. The completed shell must pass ordinary
manifold, loop, incidence, orientation and vertex-link validation. Merely stacking
unconnected prisms or overlapping shells is not accepted.

The implementation retains planar caps/floors and planar or trimmed cylindrical
walls. Bounds follow the strictly containing base, whose sidewall and full
opposite cap remain. Volume sums occupied area times slab length. Surface area
sums exposed horizontal areas and boundary perimeter times slab length. Display
tessellation is generated separately from the analytic patches and uses the real
final topology IDs. Display sampling never determines volume, area or topology.

Both end caps, all principal planes, positive/negative target extrusion and
translated tool frames are supported. Repeated/contained cuts are regularized;
annular blind tools produce connected islands supported by the floor. Existing
disjoint through-holes remain present. A later disjoint contained through-cut
rebuilds the stepped sections; a fully covering through-tool removes the body.
The earlier disjoint-pocket path, including its existing two-cap cases, remains.
New stepped solids cannot enter the uniform-prism or box envelope shortcuts.

## Identity, replay and limits

Source intervals and entity/curve ancestry remain attached to every occupied
section through the planar engine. Floors are identified by the responsible
feature relationships; side patches include their bounding floor relationships.
Shared topology identities derive from source/junction and incident patch keys,
not coordinates, display vertices or output positions. Depth edits retaining
the same floor order retain edge and face IDs. Coincident floor merges, changed
section boundaries and changed adjacency produce new relationship identities;
this is not a claim of general persistent topology naming. The application does
not offer arbitrary persistent selectors for the new stepped floors.

Existing version-5 extrusion features replay this behavior without schema changes.
Session transactions preserve accepted documents and geometry on modeling or
display failure; save/reopen and complete Undo/Redo snapshots remain in use.
Rust execution does not model or convert through OCCT. Tests construct OCCT
operands separately as an explicitly selected independent oracle.

This batch also repairs local-frame translation before a subsequent Boolean:
moving an extrusion origin must preserve its local planar coordinates. A new
regression cuts/intersects a translated body, rather than checking only its
immediate displayed shape. Contained through-cut rebuilding now retains the
original outer-boundary ancestry and adds the tool's hole ancestry.

Existing version-5 rectangular split and disjoint Join IDs keep their historical bounds ordering;
new curved outputs use source ordering. Both retain source-derived edge/face
identities and accepted-edit ancestry checks. A reversed-rectangle saved-document
regression verifies old body references and a subsequent slab operation; a mixed
circle/rectangle disjoint Join also checks saved IDs. The box
path recognizes rectangular silhouettes from their actual finite line boundaries,
including retained source subdivisions; bounding boxes alone do not establish
compatibility.

The existing 0.0000001 mm coincidence, 0.00001 mm resolution, 1e-10 radian angular
and ±1,000,000 mm domain limits apply. Distinct floors separated by more than
coincidence but less than resolution fail explicitly. Finite curve bounds prevent
distant nearly aligned supports from being mistaken for local ambiguity.
Planar provenance is bounded to 32,768 records and 64 transform instances per
source, with at most 512 source pairs per junction. The overlapping construction
permits 32 pockets, 2,048 patch curves before refinement, 262,144 refinement
events, 8,192 refined spans and 16,384 B-rep vertices/edges. More complex inputs
may hit the planar engine's existing limits earlier. Errors are explicit and
transactional; no tolerance enlargement or geometry flattening is used.

Point-contact pocket openings, sub-resolution floors, blind sidewall crossings,
opposite-side interactions on a stepped body, enclosed cavities, nonparallel
axes, and through-tools intersecting stepped pocket footprints remain unsupported.
These are bounded same-axis constructions, not general 3D curved Booleans.
Full-circle sketch crossings, splines, production Rust STEP export, complete
OCCT replacement and Fusion parity remain unestablished.

## Acceptance and evidence

`tests/overlapping_pockets.rs` checks equal and unequal radius-10 pocket depths,
both depth orderings, both caps and all principal planes, negative extrusion,
mixed line/arc footprints, existing through-holes, disjoint later through-cuts,
repetition, annular tools, analytic sections and shared topology. It compares
independently built OCCT volumes/areas and OCCT STEP reimport, checks coarse and
fine display resolutions, stable depth-edit identities, expressions, replay,
save/reopen, Undo/Redo and failed-edit retention.

For circle overlap area `L = 200π/3 − 50√3`, pocket depths `a` and `b`, and a
60×60×10 base, the independently derived volume is
`36000 − 100π(a+b) + L min(a,b)`. The surface area is
`9600 + 20π |a−b| + (80π/3) min(a,b)`. The mixed rectangle/circle fixture has a
separate circular-segment overlap expectation. Planar regressions additionally
check 27 varied-radius/translated circle pairs, commutativity and conservation,
partial coincident arc intervals and distant nearly aligned disjoint boundaries.
Prism replay additionally checks radius parameter edits through actual Booleans.

`examples/overlapping_pocket_corpus.rs` retains saved/reopened native fixtures and
backend-tagged measurements outside the repository. External evidence:
`C:/Users/Vnm4/Downloads/openfusion-reference/overlapping-pockets-2026-10-20`.
OCCT STEP results validate the oracle/interoperability path only; they do not
implement production Rust export or establish Fusion-native compatibility.

Executed `cargo fmt --all -- --check`, `cargo check --workspace` and
`cargo test --workspace -j 2`: all passed, **94 tests**. Both corpus commands
passed: `overlapping_pocket_corpus` saved/reopened **72** pocket fixtures and
`prism_corpus` rechecked **32** prism fixtures against the final source. Pocket
volume error was at most 3.7e-11 mm³, area error 1.9e-11 mm², and OCCT STEP
reimport difference 1.3e-10 mm³. `results.json`, `summary.json`, final logs and
backend-tagged measurements reside in the external directory above. Earlier
failed compatibility runs are retained under separate log names. The final
source commit is recorded externally after committing the verified tree.

No new research or distribution clearance question was required for this
independent mathematics batch. Existing research and distribution review
requirements remain applicable when those activities are separately undertaken.
