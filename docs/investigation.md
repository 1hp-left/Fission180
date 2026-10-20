# Fusion investigation and reconstruction index

Started: 2026-10-18. Updated: 2026-10-20. Target: Autodesk Fusion, previously called Fusion 360. User-selected first scope: sketching and solid modeling, within a broader full-product parity goal.

## Current status: installed Fusion and native Rust reconstruction

[Exact curved-region Rust extrusion](curved-engine-2026-10-20.md) implements analytic line/arc loops, virtual line/arc and arc/arc arrangements, trimmed cylindrical B-rep extrusion, stable source provenance and complete extrusion boundary dependencies. Version-5 saves retain versions 1–4 readability. OCCT remains the default; independent analytic, topology, replay and test-only OCCT/STEP checks are recorded separately from Fusion parity. UI development and runtime automation remain deferred.

[Analytic sketch arcs](sketch-arcs-2026-10-19.md) adds genuine bounded circular entities, radius/relational constraints, finite tangency checks and usable native three-point creation with retained valid preview. All 64 workspace tests pass; a fresh 1920×1080 native arc capture was inspected. Arc-bounded solid regions remain explicitly unavailable. The click order follows published Autodesk documentation; selected-build runtime matching remains blocked.

[Connected line arrangement and editing batch](sketch-arrangements-2026-10-19.md) implements crossing/branched line regions, retained source-boundary identities, split/merge dependency rejection and native Trim/Extend with persisted intersection constraints. Analytic cell volumes execute on explicit OCCT/Rust backends; 56 workspace tests pass. Fresh matched Fusion interactions remain blocked; this is independent implementation evidence.

[Implemented solid-command batch](features-2026-10-19.md) adds independently written application commands using explicit OCCT or scoped Rust execution: revolve, sweep, loft, hole, edge finishing, shell, face offset, transforms, patterns, Combine and Split Body. It records 17 saved/reopened native fixtures and OCCT STEP measurement checks, explicit mixed command order, face-supported sketches and dependency editing. This is implementation evidence; fresh matching Fusion command scenarios remain unverified.

[Entity editing batch](sketch-editing-2026-10-19.md) connects graph selection, constraint creation/removal, line/circle dimensions, constrained point dragging, construction state, simple closed line regions and sketch rollback display. These are independently implemented application behaviors. Matched Fusion constraint glyphs, interaction states and solving behavior remain unverified.

[Extrusion extent implementation](extrusion-extents-2026-10-19.md) replaces static start/direction/extent rows with functional controls, retains expression text, checks all principal planes on both backends and extends Rust's contained through-cut support. No Fusion taper, thin extrusion or To Object equivalence is claimed.

[Implemented integrity, sketch and kernel slice](development-2026-10-19.md) records the transactional worker/save fixes, version-1 migration/version-2 saves, sketch graph and command history, expressions/parameters, regions and split outputs, and explicit OCCT/Rust backends. Fresh clean Fusion Home/sketch/reference frames were recovered. Desktop automation was then stopped with physical Escape; matched solid and Fission UI retests remain incomplete.

The independent application was renamed **Fission 180** (formerly OpenFusion) on 2026-10-19. This branding change does not resolve the audit findings or establish parity. Active Cargo packages use `fission180-*` and the executable is `fission180.exe`. Historical observations, audit records, source directories, external evidence paths, and the `.ofrust` format identifier retain their original names for traceability and compatibility.

[Fusion architecture and kernel-ownership research, 2026-10-19](fusion-engine-research-2026-10-19.md) answers why OCCT is currently used, documents Autodesk's ACIS/ShapeManager lineage and public application contracts, distinguishes current application restrictions from demonstrated kernel limitations, and proposes a staged hand-written Rust kernel with measurable parity gates. Selected artifact hashes were rechecked and retained REA bundles validated; no new native algorithm recovery or application changes are claimed.

[Independent parity audit, 2026-10-19](audit-2026-10-19.md) rechecked the current executable/artifact identities and 125% DPI, built and operated current Rust code, reproduced a failed-Open/Save overwrite and sketch-history defects, and ran independent topology/dependency probes. Full 1:1 parity is not established. Current Fusion Preferences says Match Device Theme with dark-blue appearance. A persistent black/stale reference viewport capture blocked dependable matched-camera geometry comparisons; the audit distinguishes those blockers from observed product defects and includes a coverage matrix and next acceptance tests.

The user corrected the implementation direction: a near pixel-for-pixel Fusion interface built in Rust is required. The original TypeScript CLI was first archived, then removed from the repository's working tree at the user's explicit request on 2026-10-19. Its historical implementation remains in Git history. The active application is native Rust.

Selected executable: `C:\Users\Vnm4\AppData\Local\Autodesk\webdeploy\production\736d2b4c5e6b95e77c11721da222cf21c6a1fc1f\Fusion360.exe`.

- File/product version: **2705.1.30** (observed Windows version metadata).
- SHA-256: `ddc460d83f3fc496ff7436ceb88e65b99f3a386f29e035064430697337988a5a`.
- Actual REA Evidence: **`ev_5f1d9157d5f10b6510bc7f46801809a1b1ad77641d797b2fa084cecea5a79dfe`**, operation `inspect_pe_resources`, saved outside the source tree in `C:\Users\Vnm4\Downloads\openfusion-reference\2705.1.30\pe-resources.json`.
- Scope of that REA operation: static PE resource directory and icon-group relationships. Opaque resource payloads were not decoded. This does not establish loaded icons, running behavior, or native algorithms. No decompiler provider was installed or client configuration changed.
- Running UI was observed and operated through the Computer Use skill: Home → New Design → Part Design → Create Sketch → XY → R → 40 × 20 mm rectangle → Finish Sketch → E. This created an unsaved scratch design; existing saved designs were not edited.
- Screenshots of Sketch and Extrude states are retained alongside the external evidence. They are JPEG data despite the initial `.png` filename extension. The file initially named `design.png` records the rectangle sketch after an undo, not an empty Design state; it must not be treated as a blank reference.
- UI observations are **Computer Use observations**, not fabricated REA runtime Evidence IDs. Measured layout and outstanding differences are recorded in `docs/fidelity.md`.
- Native OpenFusion was exercised through the same plane selection, rectangle, extrusion preview and commit sequence. Native tests verify solid volumes, Boolean results, STEP round trips, rollback/history, and projection.

## Deeper REA investigation

The user requested native decompilation and internal tracing toward full product parity. The user explicitly approved portable Ghidra 12.1.4 and Java 21 under the external reference folder. Their publisher-provided archive hashes were verified. A session-local REA stdio connection now runs the Ghidra provider; global client configuration and global Java/PATH settings remain unchanged.

Actual native function dossiers now cover sketch grid/camera handling, profile color invalidation, dimension selection/highlight rendering, and grid-bound updates. [Native investigation](rea-native.md) records exact module hashes, addresses, real REA Evidence IDs and limitations. Full records and bundles remain outside this source tree. The large modeling-module Ghidra analysis exceeded both its initial 330-second and extended 1200-second startup deadlines; its owned session was closed. A bounded LLVM extrusion trace is recorded separately and carries no fabricated REA Evidence ID.

Two further completed native dossiers in `NaSketch10.dll` cover the constraint-solving request wrapper and dimension deletion bookkeeping. The separate sketch-model session was closed successfully. These establish request boundaries and some direct calls; the solver algorithm, transitive behavior and unresolved surface cleanup calls remain unknown.

The shipped artifact pass also identified sketch/solid toolbar declarations, QML command dispatch, dimension field draft/evaluation/reset declarations and candidate native command handlers. Static QML presence does not prove that those controls are active in the observed running workflow.

[Extrude invalid-input scenario](runtime-extrude-input.md), local observation `RUNTIME-002`, records a live incomplete HUD expression, preservation of the linked last-valid value, and whole-command cancellation on Escape. Its captures are 1202 × 631 logical pixels. Finish Sketch retained Top view in this pass, so the initial isometric-exit observation is not a universal rule.

On 2026-10-19, `RUNTIME-003` accepted `2 * 3 mm` on the same 40 × 20 mm profile and reported one body, volume 4800.00 mm³ and area 2320.00 mm² in Fusion Properties. An independent Rust fixture and measured-volume comparison test are linked from that scenario. Expression-entry parity remains incomplete.

[Parity ledger](parity.md) tracks the full product target and the first reproducible comparison scenarios. General constraints, native extrusion execution, failure details in Fusion, advanced workspaces, service behavior and matched performance remain incomplete. This investigation is not complete.

## Bootstrap history

## Historical bootstrap evidence

| ID | Source | Observation | Boundary |
| --- | --- | --- | --- |
| ENV-001 | Workspace directory inspection and `git status` | The initial OpenFusion directory was empty and not a Git repository. | Local workspace only. |
| ENV-002 | Installed skill and `rea doctor --client codex --json` | REA CLI 6.7.0 and skill version 42 are installed. No REA tools are exposed in this chat. Doctor reports missing Codex registration and unavailable Hopper/Ghidra/IDA providers. | Environment diagnostics, not evidence about Fusion behavior. |
| ENV-003 | Bounded Autodesk install-directory, process, uninstall-entry and shortcut checks | Autodesk user data and old deployment logs exist; a current Fusion executable was not located in the checked places. | A bounded search, not proof of absence everywhere. |
| USER-001 | User message in this chat | The user confirms Fusion was deleted and intends to reinstall it. | No new target path or version selected yet. |
| BUILD-001 | Automated tests in `test/model.test.ts` | OpenFusion constructs and edits solids with checked analytic volumes; STEP round trips and STL structure are checked. | Independent implementation evidence. Does not establish Fusion compatibility. |

These bootstrap records are local environment/build observations, **not REA Evidence IDs**. At bootstrap no Fusion package, executable, runtime action, native function, or internal algorithm had been analyzed. The current status above supersedes the installation/target unknowns below.

## Bootstrap questions at the time

| Question | Status | Evidence needed |
| --- | --- | --- |
| What exact Fusion build is the reference? | Unresolved; awaiting reinstall | Executable path, file version, SHA-256, selected associated artifacts. |
| What state transitions occur from plane selection through sketch completion? | Unresolved | Bounded runtime observation and a saved action sequence. |
| How are constraints, dimensions, and underconstrained states exposed? | Unresolved | Small reproducible sketches; record state before/after each edit. |
| How do new-body, join, cut, and intersect extrusion modes behave? | Unresolved | Reproducible input profiles, distances, target selection, and resulting body measurements. |
| What happens after changing a driving dimension or suppressing a feature? | Unresolved | Dependency/order behavior and failure/rollback observations. |
| Which neutral formats and units preserve our initial test part? | Partially answered for OpenFusion | OpenFusion STEP import round trip passes. Fusion-side export/import comparison remains open. |

## Original resumption protocol

1. Resolve the actual executable and build. Hash the exact selected artifact and retain analysis output in a separate local evidence directory.
2. Use a narrow inventory appropriate to the supplied artifact. Native decompilation requires a supported configured provider; the CLI alone does not provide one. Static package/resource inspection may be possible without it.
3. Observe a small task: centered 40 × 30 mm rectangle → 5 mm extrusion → change width to 80 mm → add a 10 mm diameter hole → undo/redo. Record actions, resulting dimensions, body counts, and error behavior.
4. Compare observable results with OpenFusion's analytic tests. Add tests for discrepancies before extending the implementation.
5. Link actual REA Evidence IDs and their limitations here. Keep inferred internals separate from observed UI or geometry.

## Optional connection repair, prepared but not applied

`rea setup --client codex --dry-run --json` proposed one action:

- Update `C:\Users\Vnm4\.codex\config.toml` with the REA MCP registration, preserving unrelated configuration.
- Back up the inspected configuration to `C:\Users\Vnm4\.codex\config.toml.rea.backup`. REA may choose a different recovery path if the configuration changes before setup runs.
- Launch the installed REA public entry point through `C:\Program Files\nodejs\node.exe` and `C:\Users\Vnm4\AppData\Roaming\npm\node_modules\rea-agents\scripts\rea.mjs`, argument `mcp`.
- No provider installation, environment additions, or skill replacement was proposed by this dry run.

No client configuration was changed. The CLI remains usable for its supported operations. Apply this registration plan only if authorized and needed; reconnect afterward and verify tools actually appear. Native engines are a separate prerequisite.
