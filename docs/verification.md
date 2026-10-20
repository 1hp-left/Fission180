# Native desktop verification

## Current implementation — 2026-10-19

The latest source passes fmt/check and **47 workspace tests**, with new independently checked sketch editing, command replay, persistence, topology and extrusion extents. [Integrity/kernel repairs](development-2026-10-19.md), [solid commands](features-2026-10-19.md), [entity editing](sketch-editing-2026-10-19.md), and [extent evidence](extrusion-extents-2026-10-19.md) record exact scope, backend execution, saved/reopened fixtures and inspected native screenshots. Full parity remains unachieved. The seven-test rename baseline below is historical.

## Fission 180 rename — 2026-10-19

The active Rust app, Cargo packages, executable, window title, Home text, file-dialog labels, and initial badges now use Fission 180 / `fission180`. Source paths, historical audit evidence, and existing version-1 `.ofrust` documents retain their original names/format for compatibility. `FISSION180_TIMINGS` is the current timing switch; `OPENFUSION_TIMINGS` remains an alias.

`cargo fmt --all -- --check`, `cargo check --workspace`, and `cargo test --workspace` passed after the rename (seven tests). A release build and framebuffer capture verify the visible branding at 1920 × 1080 / 125% scaling. Capture: `output/fission180-design.png`.

This verifies the rename only. The [independent audit](audit-2026-10-19.md) remains the source for reproduced persistence/sketch-history defects and missing functionality. Matched-camera geometry comparisons remain blocked by Fusion's black/stale viewport capture. The measurements below are historical checks, not new parity acceptance results.

## Earlier baseline

Verified on Windows, baseline 2026-10-18 and measured-reference follow-up 2026-10-19, using Rust 1.97.1 and the installed Fusion 2705.1.30 reference.

- `cargo fmt --all -- --check`: passed.
- `cargo check --workspace`: passed.
- `cargo test --workspace`: seven native tests passed. These cover analytic extrusion volumes, cut/join/intersection, STEP round trips, invalid-edit rollback, undo/redo, three sketch planes, negative distances, polygon rejection, document versions, viewport coordinate projection, Ctrl-key chords whose modifiers are released within the same frame, and the measured Fusion reference below.
- Historically, the TypeScript prototype passed its type check and 15 tests. That prototype has since been removed from the current tree at the user's request; it was never linked to the Rust application.
- Native UI smoke sequence: Create Sketch → XY plane → R → corners at the origin and 40 × 20 mm → E → distance 10 mm preview → OK. The native worker committed a solid and updated Browser/timeline state.
- The final optimized executable was manually checked with Ctrl+Z and Ctrl+Y: undo removed the demo document's solid and timeline features; redo restored them. The release window was left open with the sample solid.
- Release framebuffer captures were generated for blank Design, blank Sketch, and a committed solid at 1920 × 1080 physical pixels / 125% display scale.

## Measured Fusion reference

Fusion accepted `2 * 3 mm` for extrusion of the previously drawn 40 × 20 mm profile. Its Properties panel reported one body, volume 4800.00 mm³ and surface area 2320.00 mm². [The runtime scenario](runtime-extrude-input.md) retains the actions and external screenshot paths.

The independent `.ofrust` fixture stores the evaluated distance of 6 mm. The Rust integration test compares one body, volume and kernel-measured surface area against the displayed two-decimal measurements (tolerances 0.005 mm³ and 0.005 mm²), analytic extents within 0.00001 mm, and STEP round-trip volume within 0.00001 mm³. Fission 180's expression editor and matching Properties interface remain incomplete.

## Small-workload performance baseline

`cargo run --release -p fission180-core --example native_benchmark` measures native construction of a 40 × 30 × 5 mm block, a 10 mm diameter through-bore cut, and tessellation. It also verifies analytic volume and a STEP round trip.

| Measurement | Result |
| --- | --- |
| Cold iteration | 12.187 ms |
| Warm iterations | 20 |
| Warm median | 8.863 ms |
| Warm minimum | 8.739 ms |
| Warm maximum | 9.836 ms |
| Expected / checked volume | 5607.300918 mm³ |

This is a single-host development measurement of a small part, taken while other development work was running. It is not an isolated performance study or evidence about large assemblies, complex constraints, or sustained viewport frame rate.

## Visual comparison

The retained Fusion empty Sketch reference was compared with the Rust Sketch framebuffer normalized to 1536 × 864 logical pixels. Mean absolute RGB error on the 0–255 channel scale was 15.894 for the ribbon, 12.272 for the Browser region, 7.602 for the Sketch Palette, and 3.043 for the main viewport region. Reference JPEG compression, native PNG encoding, differing font renderers, and original icon approximations affect these figures.

The local comparison sheet is `output/fidelity-comparison.png`, with the raw metrics in `output/fidelity-metrics.json`. These values are inspection aids, not a claimed percentage of Fusion parity or a passed pixel-perfect quality threshold. Remaining differences are tracked in `docs/fidelity.md`.
