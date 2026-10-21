# Generalized same-axis solid Booleans — 2026-10-20

Independent Rust geometry development after `1299bd0`. The checkout, clean main
branch and GitHub origin/main were verified at that commit before editing. No
Fusion investigation, imported Autodesk assets, new dependencies, browser runtime
or OCCT conversion was used. OCCT remains the default backend; Rust never falls
back to it. Analytic/OCCT comparisons below are independent implementation checks,
not Fusion feature or visual parity.

## Occupied sections and exposed boundaries

The reusable `kernel/native/steps.rs` path implements Cut, Join and Intersect of
parallel principal-axis analytic extrusions and earlier section results. Both
operands normalize into one principal-plane frame using their real origins and
occupied intervals. Every actual axial event partitions the operands; each open
interval uses analytic line/arc planar Difference, Union or Intersection. Empty
intervals remain during connectivity analysis, so a full removed slab separates
the output instead of being bridged by its former envelope.

One graph node represents an occupied region on an interval. Adjacent intervals
connect only across a positive-area analytic intersection. Multiple regions can
belong to one solid through a supporting floor or later bridge. Point/edge-only
attachment at a shared axial event is explicitly rejected. The completed B-rep
also rejects pinched vertex links, unmatched/co-oriented edges and disconnected
boundary shells, including enclosed cavities. This does not support multi-shell
solids or arbitrary curved surface intersections.

Only exposed horizontal differences become caps/steps. Every occupied interval
contributes its outer and hole walls. The retained analytic patch assembler
refines line/arc junctions, shares vertices/edges, and binds opposed coedges.
Internal horizontal interfaces are absent. Coplanar/cocylindrical face partitions
remain and can produce the already documented display seams.

Volume is the sum of occupied area times interval length. Area is the sum of
boundary perimeter times length and exposed horizontal areas. Bounds come from
actual occupied sections and exact curve extrema. Tessellation is display-only;
coarse/fine meshes cannot change classification, topology or measurements.

The former shape-specific pocket metadata and original-envelope equality shortcut
were removed. Uniform prism and legacy box paths exclude section solids.
Translation rebuilds the real occupied sections, preserves local planar geometry,
and adds semantic transform ancestry. `planar_region()` returns no uniform
silhouette for stepped solids, preventing use of the original extrusion profile
as authoritative geometry.

## Identity, persistence, transactions and limits

Axial events retain original feature/region/end relationships and transform
instances. Planar spans retain the existing source, junction and directed interval
ancestry through splits/coincident merges. Face/edge identities and new body
signatures use those semantic relationships rather than coordinates or output
ordinals. A cutter ending on an exterior target cap has the same pocket opening
identity as one extending beyond it. Depth edits retaining the event order and
section relationships retain references. Changed topology/ancestry in an accepted
session requires explicit dependency repair.

Historical v5 rectangular split and disjoint Join bounds-order contracts remain,
including axial splits. New section outputs otherwise order by semantic section
signatures. Old v1–4 reading/v5 saving remain unchanged; no kernel handles or mesh
geometry are persisted. Rebuild and meshing failures retain the accepted document,
model, display, Undo/Redo state and save destination. Shapes remain owned by Rust
RAII, including independently built OCCT test oracles. Native worker edits and
queued saves retain the existing serialized transaction path.

Existing policies remain: coincidence 1e-7 mm, resolved separation 1e-5 mm, angular
1e-10 rad and finite ±1,000,000 mm domain. Distinct events between coincidence and
resolution fail; no increased tolerance or healing was introduced. New bounded
section resources are 64 axial levels and 512 occupied cells. Existing bounds
remain 512 planar input curves, 32,768 planar events, 8,192 spans, 2,048 patch
curves, 262,144 refinement events and 16,384 B-rep edges/vertices. Section/exposed
patch provenance is bounded at 32,768 records, junction ancestry at 512 source
pairs, transform depth at 64 instances and solid history at 32,768 operations.
More complex operations can reach a stricter existing limit first.

## Acceptance and native application

Reusable specifications are in `examples/support/axial_fixtures.rs`, regression
checks in `tests/axial_booleans.rs`, and saved/reopened evidence generation in
`examples/axial_corpus.rs`. Existing numeric regressions remain. Former rejection
assertions were changed only where the newly supported sidewall/Join behavior
now has verified geometry; cavity, tangency and invalid-edit failures remain.

Independent expected measurements include:

| Design | Expected volume (mm³) |
| --- | --- |
| 40×20×10 box; r5 notch centered (20,0), z10→7 | `8000 − 37.5π` |
| Continue with r1 through-hole centered (20,2) | `8000 − 44.5π` |
| Through-hole centered (20,4.5), partially crossing notch | `8000 − 47.5π + 3 lens(5,1,4.5)` |
| r10 disks, centers −5/+5, intervals [0,10]/[5,15], Join | `2000π − 5L` |
| Same unequal disks, Intersect | `5L` |
| Opposite r10 pockets, depths 3/3, 5/5, 6/6 | `36000 − 200πd + max(0,2d−10)L` |
| Remove the entire box footprint over [4,6] | Two solids of 3200 each |

Here `L = 200π/3 − 50√3` and `lens(r,s,d)` is the standard analytic disk overlap
area. Separate expectations cover areas, mixed line/arc blind sidewall tools,
annular supported islands, axial gaps and full removal. All principal planes,
translated tool frames, both signs and reversed directions are covered. Other
checks include chained stepped operands, equal envelopes with different voids,
conservation, identity, rejoining axial gaps with/without a bridge, source-stable
depth edits, changed-output failures, save/reopen, Undo/Redo and injected meshing
rollback. Repeated stacking and transform growth deliberately hit resource limits.
OCCT operands are built independently from fixture parameters; OCCT STEP
round-trips test only the oracle/interoperability path.

The File → Rust Engine Examples menu now adds **Opposite-cap breakthrough
pockets** (`--engine-example breakthrough`) and **Unequal-height circle Join**
(`--engine-example boss`). Both use ordinary editable sketches/extrusions and
Combine, the geometry worker and actual displayed-engine status. Breakthrough
has offset radius-10 pockets entering opposite caps to 6 mm depth. Its bottom
pocket can be edited to 5.5 mm while retaining topology. The unequal Join uses
10 mm extrusions starting at z0/z5; edit Tool extrusion to 9 mm. Preview, OK,
Cancel, Undo/Redo and save/reopen pass native editor/worker tests.

External evidence directory:
`C:/Users/Vnm4/Downloads/openfusion-reference/axial-booleans-2026-10-20`.
## Verified results

All requested commands passed on the final Rust source:

- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo test --workspace -j 2`: **110 tests** (90 core, 20 desktop), plus rustdoc.
- `cargo build --release -p fission180-desktop -j 2`

The final `axial_corpus` run saved/reopened **312 fixtures**: 13 scenarios, three
planes, two signs, both direction conventions and two explicit backends. All
Rust boundary shells validated, including every disconnected output. Maximum
analytic volume error was 7.3e-12 mm³, analytic area error 9.1e-12 mm²,
Rust/OCCT volume difference 1.5e-11 mm³ and area difference 7.3e-12 mm².
OCCT STEP reimport differed by at most 2.3e-10 mm³. Acceptance tolerance is
1e-5 for measurements (2e-5 for OCCT-expanded bounds).

Fresh release instances produced and visually verified `breakthrough.png`,
`boss.png` and `occt-default.png` with corresponding JSON measurements. They
report 1920×1080 physical pixels and 1.25 native pixels per point. No system DPI
setting changed. The default example still reports OCCT, volume 18000 mm³.
The two new examples report Rust and the independently expected measurements:

| Runtime design | Bodies | Volume (mm³) | Area (mm²) |
| --- | ---: | ---: | ---: |
| Opposite-cap breakthrough, 6/6 mm | 1 | 32475.7627554140 | 10024.5324930441 |
| Unequal-height circle Join, 10/10 mm | 1 | 5669.00045787521 | 2058.16067291076 |

A separately launched fresh release window was operated with the Computer Use
skill. File-menu loading of both new examples, timeline double-click editing,
live preview, Cancel, OK, Ctrl+Z and Ctrl+Y were visually verified. The
breakthrough bottom-depth edit to 5.5 mm changed the displayed volume to
32571.42 mm³; Cancel/Undo restored 32475.76. The Join tool-depth edit to 9 mm
changed volume to 5354.84 mm³; Cancel/Undo restored 5669.00. Redo restored both
edited results. Every displayed state reported Rust analytic (and Preview only
while the command was active).

Both edited designs were saved through the native Save dialog to new external
`ui-*-edited.ofrust` files. Fresh-process `--open` captures then reported the
unrounded expected volumes 32571.4239031631 and 5354.84119251623 mm³, one Rust
body, no errors, no preview and no unsaved changes. The owned interaction window
and capture instances closed; no pre-existing user window was controlled or
closed. The earlier automatic default capture launch had malformed joined
arguments; its trial log is retained, the script was corrected and all three
captures then passed. Native shell validation/mesh failures are covered by the
worker tests; these UI observations are separate runtime evidence.

`capture-summary.json`, `corpus-summary.json`, `ui-replay-summary.json`,
`ui-evidence.json`, the measurements and PNG/JPEG states retain the backend,
parameters, expected/measured values, body counts, bounds and replay evidence.
`verified-rust-source.json` records the tested source hashes; `source-record.json`
records the final commit and executable digest outside the self-referential
source tree. Final command logs are `fmt.log`, `check.log`, `test.log`,
`build-release.log` and `corpus.log`. Earlier failed trial logs remain separately.
The first baseline command's 99 test bodies passed but its rustdoc phase
overlapped source edits; it is not claimed as a complete passing baseline run.

Before the release build, package-scoped development cache cleanup removed
5.4 GiB of regenerable Fission core/desktop artifacts. OCCT/dependency artifacts,
source, evidence and user files were retained. The release build then passed.

Existing coplanar side seams and edge occlusion remain visible; these screenshots
do not establish Fusion parity or complete viewport fidelity.

## Remaining scope

Nonparallel curved operations, arbitrary rotated supports, splines, full-circle
sketch arrangement discovery, enclosed cavities with multiple boundary shells,
revolve/fillet/chamfer/shell in Rust and production Rust STEP exchange remain
unsupported. Existing scoped legacy box operations are retained. This is bounded
same-axis analytic solid modeling; it does not establish full Fusion parity.
