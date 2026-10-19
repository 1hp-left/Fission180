# Fission 180

Formerly OpenFusion. The active Cargo packages are `fission180-core` and `fission180-desktop`, and the executable is `fission180.exe`. The `openfusion-native` identifier is retained. Version-1 documents have explicit in-memory migration; new saves use version 2 for stable references, constraints and expressions. Source directories and external evidence paths retain their original names so audit links remain usable. The archived TypeScript prototype retains its historical identity.

The [Fusion architecture and kernel research](docs/fusion-engine-research-2026-10-19.md) explains the current OCCT choice and the proposed path toward an independently written Rust kernel while pursuing measured 1:1 Fusion behavior.

A native Rust desktop CAD application being reconstructed against the installed Autodesk Fusion interface. The target is full visual and behavioral parity with the selected Fusion desktop build, starting with sketching and solid modeling. This is the development requirement, not an achieved result.

The current reference is **Fusion 2705.1.30**, with **Match Device Theme**, visibly dark blue, at **1920 × 1080 and 125% Windows display scaling**. This is an early working reconstruction, not full Fusion feature parity. The [2026-10-19 audit](docs/audit-2026-10-19.md) reproduced a failed-Open/Save file-overwrite defect and sketch undo/redo defects; source repairs and regression tests are documented in [the implementation record](docs/development-2026-10-19.md). Unit expressions, stable sketch regions, split bodies, an initial constraint solver and an explicit experimental Rust kernel are now implemented. Many workspaces and workflows remain incomplete. Fresh clean Fusion reference captures were recovered, but desktop verification was interrupted when Computer Use was stopped. See [visual fidelity and gaps](docs/fidelity.md).

## Run the desktop app

```powershell
cargo run --release
```

Or launch the compiled application:

```powershell
.\target\release\fission180.exe
```

The application runs natively. There is no Node.js process, browser, or WebView in its runtime. Rust owns application state, input, persistence, history, and rendering. OpenCascade 8.0.1 through `cadrum` remains the default solid backend. The File menu explicitly selects OCCT or the limited independently written Rust analytic backend. Unsupported Rust operations return errors without OCCT fallback.

Build requirements: Rust 1.95 or newer and a Windows MSVC C++ toolchain. The first Cargo build downloads dependencies and the binding's prebuilt OpenCascade archive. Both lockfile and kernel binding version are pinned. Development was verified with Rust 1.97.1 on Windows.

## First modeling workflow

1. Click **Create Sketch**, then choose an origin plane.
2. Press **R** and click two corners to draw a rectangle, or **C** to place a circle center and radius. **L** creates open line entities; Escape ends the chain. The Polygon tool creates closed polygons.
3. Press **D** to edit the selected rectangle's width/height or circle radius.
4. Click **Finish Sketch**, then press **E**. Pressing **E** while sketching also finishes the sketch and opens Extrude.
5. Enter a length expression such as `2 * 3 mm` or `1 in`, choose New Body / Join / Cut / Intersect, and click **OK**. Changes preview using native solid geometry.
6. Use **Ctrl+Z / Ctrl+Y** for model undo/redo. Scroll to zoom; middle drag pans; Shift + middle drag orbits. Click the view cube to toggle Top/Home.
7. Use the File menu to save/open `.ofrust` documents or export STEP/STL.

Geometry work and meshing run on a dedicated worker thread. A failed geometry edit retains the last valid committed model. Display meshes and projected triangles are cached; the event-driven window does not continuously redraw while idle. Dimensions are millimetres.

Double-click a sketch in the Browser or timeline to edit it. The next geometry commit replays its dependent extrusions. Sketch Undo/Redo reverses complete accepted edits, including dimension expressions and constraints. Double-click an extrusion timeline item to edit/replay it.

## Current scope

The desktop shell reproduces the observed title/quick-access bars, workspace tabs, ribbon groups, Browser tree, view cube, grid, navigation strip, timeline, Sketch Palette, plane selection, and docked Extrude layout. Icons are independently drawn vectors; they are not Autodesk image assets.

Working modeling features: rectangles, circles, closed polygons, XY/XZ/YZ sketches, positive/negative extrusion, new bodies, union/cut/intersection, dimensional edits, feature replay, local save/load, STEP/STL export, and model undo/redo.

Many ribbon controls are visual scaffolding. Unsupported actions have an explanatory tooltip or disabled menu entry. Fillets, revolutions, shelling, assemblies, CAM, simulation, face-based sketches and Fusion document import are not implemented. Initial graph constraints and Rust topology picking exist; general constraint editing and robust OCCT topology naming are incomplete. Boolean features handle empty and multiple results with persisted output references. The home screen is only a local-workspace scaffold.

## Verification

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo run --release -p fission180-core --example native_benchmark
```

Tests check analytic solid volumes, real Boolean operations, STEP export/import round trips, failed-edit rollback, model history, plane orientation, and viewport coordinate projection. The benchmark measures a small extrusion + bore cut + tessellation workload; it does not establish performance on large assemblies.

Set `FISSION180_TIMINGS=1` to print frame timings; `OPENFUSION_TIMINGS` remains accepted for existing scripts.

Reproducible screenshot commands:

```powershell
.\target\release\fission180.exe --capture output/design.png --exit-after-capture
.\target\release\fission180.exe --sketch --capture output/sketch.png --exit-after-capture
.\target\release\fission180.exe --demo --capture output/solid.png --exit-after-capture
```

## Source

- `crates/openfusion-desktop/`: native UI, vector icons, viewport, and worker integration.
- `crates/openfusion-core/`: validated documents, native solids, replay, and history.
- `docs/investigation.md`: reference build identity, observations, and REA evidence.
- `docs/rea-native.md`: Ghidra function findings, exact evidence, and reconstruction limits.
- `docs/parity.md`: product coverage and pending comparison scenarios.
- `docs/fidelity.md`: measured visual target and remaining differences.
- `legacy/typescript-prototype/`: archived initial CLI experiment. It is not used by the Rust app.

Original Fission 180 code is MIT licensed. OpenCascade and other dependencies retain their own licenses; see [third-party notices](THIRD_PARTY_NOTICES.md). This is an independent application.
