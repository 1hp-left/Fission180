# Rust engine in the desktop app — 2026-10-20

This batch follows engine commit `2d3df38`. Extrude and Combine already dispatched
through the explicit kernel boundary; the release executable on disk predated
the latest engine work. This batch makes the Rust path discoverable, rebuilds the
actual executable, and exercises it through the native geometry worker and view.
OCCT remains the default for new documents. No Fusion runtime or existing user
window was controlled, and no UI parity claim is made.

## Trying the engine

Launch `target/release/fission180.exe`, then choose **File → Rust Engine
Examples…**. Available designs are:

| Example | Ordinary editable features | Expected result |
| --- | --- | --- |
| Sidewall notch | Rectangle extrusion and circular Cut | One notched body, volume `4000 − 62.5π` mm³ |
| Circle Join | Two circle extrusions and Combine | One curved union |
| Circle Intersect | Two circle extrusions and Combine | One lens-shaped body |
| Split cut | Rectangle extrusion and strip Cut | Two bodies, total volume 3800 mm³ |
| Annular cut | Disk extrusion and annular Cut | Outer ring and center cylinder, total volume `225π` mm³ |
| Overlapping blind pockets | Base extrusion and two cap-entering Cuts | One body with 3 mm and 5 mm pocket depths |

These are version-5 parametric documents created from sketch profiles and features,
not baked display meshes. They execute on Rust. The viewport footer names the
engine that actually produced the current display, including previews and empty
results. The File menu checks the accepted document's selected engine. Measure
also reports each measured body's backend.

Double-click a timeline feature to edit it. For the pocket example, edit
**Pocket 2 — 5 mm** and change the signed distance from `-5` to `-4 mm`; preview,
OK, Cancel and Undo/Redo use the normal native command/worker path. For a blank
design, choose **File → Use Rust Analytic Kernel**, then sketch and extrude normally.
Unsupported operations return errors and retain the accepted design; there is
no OCCT fallback.

Loading an example is an undoable whole-design replacement and detaches the prior
file's save path. It cannot implicitly save over the previous file. The explicit
whole-design replacement operation has a separate identity namespace; ordinary
edits retain ancestry validation. Failed modeling or meshing does not replace
the old document, model, display, history or save path. Fresh examples fit into
the viewport without changing system DPI.

The native Extrude editor now preserves saved outer/hole boundary dependencies
when changing a distance on the same region. Its preview/OK document builder is
tested against real Rust pocket replay and a newly introduced hole; the latter
requires explicit repair and cannot silently rebind the feature. Existing split
body references remain attached to edits.

## Runtime and regression evidence

The desktop worker tests exercise all six examples, exact analytic volumes,
zero/one/multiple bodies, actual backend tags, native mesh face identities,
saved/reopened documents, cross-example Undo/Redo, preview cancellation, accepted
depth edits and failed-edit retention. They also exercise successful OCCT-to-Rust
selection and a rejected switch for an OCCT fillet, preserving the saved file.
Camera fitting is tested on translated geometry with space for the existing
Browser and command palettes.

Required checks passed on this source:

- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo test --workspace -j 2`: 99 tests passed (81 core, 18 desktop).

The desktop tests include the actual Extrude editor's document builder, not just
hand-constructed replacement documents. The analytic engine assertions remain
independent of display triangulation.

Actual native capture command:

```powershell
.\target\release\fission180.exe --engine-example pockets --capture <external.png> --capture-report <external.json> --exit-after-capture
```

Other example keys are `notch`, `join`, `intersect`, `split` and `annular`.
`--capture-report` records document and displayed backends, body IDs/count,
kernel-measured volume/area/bounds, triangle count, preview/error state, native
pixel scaling and screenshot dimensions. It reports the accepted/preview display
from the ordinary app worker rather than reconstructing a separate test result.

Generated screenshots, reports, build and check logs reside outside the source
tree under:
`C:/Users/Vnm4/Downloads/openfusion-reference/desktop-engine-2026-10-20`.

The final `cargo build --release -p fission180-desktop -j 2` passed. Eight native
captures from that executable passed: all six designs, pocket Measure, and the
live pocket Extrude preview. Each reported the Rust document, display and body
backends, no modeling error, and the expected body count and analytic volume
within `1e-5 mm³`. All captured at 1920 × 1080 with the unchanged native scale
of 1.25 pixels per point. Screenshots were visually inspected as well.
An additional `--demo` capture (`occt-default.png` / `.json`) retained the default
OCCT document, display and body backends and measured its 40 × 30 × 15 mm box at
18000 mm³ with no error.

| Runtime design | Bodies | Measured total volume (mm³) |
| --- | ---: | ---: |
| Sidewall notch | 1 | 3803.65045915064 |
| Circle Join | 1 | 2527.40780428541 |
| Circle Intersect | 1 | 614.184849304379 |
| Split cut | 2 | 3800 |
| Annular cut | 2 | 706.858347057704 |
| Overlapping blind pockets | 1 | 33855.2367867108 |

`capture-examples.ps1` retains the runtime assertions and capture procedure;
`capture-summary.json` records expected/measured values, tolerance and executable
SHA-256. Per-design JSON contains body IDs, areas, bounds and triangles. Check
logs are `fmt.log`, `check.log`, `test.log` and `results.json`; the successful
release log is `build-release-final.log`. `source-record.json` records the final
Git source identity separately, avoiding a self-referential commit hash.

The first final release rebuild exhausted disk space during archive creation.
Package-scoped `cargo clean -p fission180-core -p fission180-desktop --profile dev`
removed 9.4 GiB of regenerable development build artifacts. This retained the
OCCT installation/dependency artifacts, source, documents and external evidence.
The failed build log is retained as `build-release-disk-failure.log`.

The overlapping-pocket display currently shows extra edges around the outside
walls at pocket-floor levels. These are retained coplanar B-rep face partitions;
the validated model is one solid with the correct measured area and volume.
Suppressing those seams in the display remains a visual-polish task.
The annular screenshot also exposes some body edges through foreground faces;
edge occlusion remains a viewport gap. These captures validate this integration's
runtime behavior and document the display gaps; they are not Fusion comparisons.

The mathematical limits from [planar/prism operations](planar-prisms-2026-10-20.md)
and [overlapping pockets](overlapping-pockets-2026-10-20.md) continue to apply.
This integration does not add arbitrary face supports, general 3D Booleans,
nonparallel/opposite-side pocket operations, Rust STEP export, complete topology
naming, or Fusion workflow/visual parity. No new runtime dependency was added.
