# Extrusion extents — 2026-10-19

The formerly static Start, Direction and Extent rows now edit persisted feature options. The Profiles selector changes the selected sketch region. The normal worker preview, commit, timeline edit and Undo/Redo paths execute the resolved operation.

- Profile Plane or Offset start; offsets accept unit expressions and named parameters.
- One Side, including negative distances; Symmetric with the entered distance interpreted as the whole length; Two Sides with independent positive expressions.
- Distance or All, with an explicit target for All. The target's analytic bounds determine a sufficient axial interval and the actual solid Boolean determines the result. A small overrun avoids a coplanar terminal cap. A target behind a one-sided direction is rejected.
- Parameters are evaluated during replay. Offset/second-side expression text persists. Empty/incomplete active input cannot commit; failed modeling retains accepted state and last valid display.

Both **OCCT** and explicit **Rust analytic** extrusion execute these intervals. Rust's contained circular/polygon through-cut now accepts a cutter whose axial interval covers the entire target, with aligned planar frames; it no longer requires identical start and end planes. It constructs a genuine analytic region-with-holes B-rep, with operation provenance, without OCCT fallback. General curved/partial-depth intersections remain unsupported.

`tests/extrusion.rs` checks all XY/XZ/YZ planes, both backends, positive/negative through-cuts, upstream depth edits, offset and second-distance expressions, symmetric whole-length bounds, saved/reopened JSON, backwards directions and invalid-draft rollback. These are Fission semantics checked against analytic geometry; fresh corresponding Fusion runtime observations are still pending.

The expanded `acceptance_corpus` saved and reopened **14** native fixtures in external `C:/Users/Vnm4/Downloads/openfusion-reference/development-2026-10-19/extent-fixtures`, with backend-tagged volume, area and bounds in `measurements.json`. The new cases are offset-symmetric, two-sided and through-all on both backends. The original eight-fixture corpus remains separately retained.

`cargo fmt --all -- --check`, `cargo check --workspace`, and `cargo test --workspace -j 2` passed; the workspace executes **47 tests**. Retained logs are `extent-fmt.log`, `extent-check.log`, `extent-test.log`. The native `fission180-extents.exe` was built and operated with the saved two-sided Rust fixture. The inspected **1920×1080** `extent-two-sided-palette.png` shows Offset using the named parameter `offset`, Two Sides, 4 mm and 6 mm distances, disabled taper, actual solid preview and 12000.00 mm³ status. Capture arguments: `--open <two-sided-rust.ofrust> --command 'Edit Extrude' --capture <path.png> --exit-after-capture`. This validates native rendering and worker replay; interactive mouse acceptance and matched Fusion extent states remain unverified.

To Object, taper, thin extrusion, multiple simultaneous profile selection, all Fusion extent measurement modes and exact palette/selection/camera parity remain absent. Taper is shown disabled with an explicit unsupported tooltip. Symmetric whole-length behavior is an application choice until matched against the selected reference.
