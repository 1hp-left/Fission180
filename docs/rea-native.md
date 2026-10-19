# Native Fusion investigation

## Scope and provenance

This pass uses the installed reverse-engineer-anything skill and REA 6.7.0 against the selected Fusion 2705.1.30 installation. Native analysis uses Ghidra 12.1.4 with a portable Temurin Java 21 JDK, approved by the user in this chat. Only process-local `GHIDRA_INSTALL_DIR` and `JAVA_HOME` settings are supplied. Codex configuration, global PATH and the existing Java installation are unchanged.

Portable tools live under `C:\Users\Vnm4\Downloads\openfusion-reference\tools`. Archives were checked against publisher metadata before extraction:

| Archive | SHA-256 |
| --- | --- |
| `ghidra_12.1.4_PUBLIC_20260921.zip` | `ddac49f903da9d5bac833e5cc79395098b9c33cfd3279be5f31bd00387d2d4db` |
| `OpenJDK21U-jdk_x64_windows_hotspot_21.0.12.1_1.zip` | `f9d6e191ab098c0d416e7d588a24420a8621cd2f4720dab2459b8b7b2d2d8b4e` |

Full target-derived records are outside the source tree at `C:\Users\Vnm4\Downloads\openfusion-reference\2705.1.30`. `native-session` contains complete REA tool responses and exported bundles. A local driver connects to REA's public stdio entry point, reads its advertised schemas, and reuses one target session for related queries. It does not register an MCP server in Codex. No recovered Autodesk code or assets are used as Fission 180 source.

## Shipped UI inventory

The external `artifact-map-report.md` and `artifact-map-hashes.json` retain 23 selected artifact identities and exact source locations. Ordinary XML/source observations are labeled separately from REA records.

`Fusion/UI/FusionUI/Resources/Toolbar/TabToolbars.xml` declares the sketch and solid command groups. Its SHA-256 is `aa1e18f8ee96f79568b67da9ee8707c4ff1ed2688b60eefa15bc4ce3b654761f`. Six selected panels were parsed; declarations can be hidden by runtime availability rules.

Shipped QML separates command visibility, enabled state and active state. Dimension-related fields retain draft text, expression text and evaluated values, evaluate edits, and declare Escape restoration of the linked value. These are static declarations. Which QML versus legacy native controls are active in the observed workflow remains unknown.

The separate [runtime input scenario](runtime-extrude-input.md) observed a HUD draft of `5 +` alongside a palette value of `5 mm`. Escape closed the command in that scenario. A field-level QML handler must not be treated as the complete application Escape-routing contract.

REA helper analysis: `ev_dcfe3705627ee9b50fbd40104edfd7ca821f36e900003ad8b559b0342e9d02d9`, scoped to two JavaScript helper files, 19,266 text bytes, no parse failures. This does not analyze QML or native command execution.

## Native sketch UI

Target: `Applications/DirectModeling/NaSketchUI10.dll`, 330,816 bytes, x86-64 PE, SHA-256 `a26abc00c8ad1f39518bc9de253c6abdbfec3e3c2cf1e3da8351bd69f318183d`.

REA classification `ev_15787325c66cc1eda3419f74161c69a05bbd4ff692aeae61b7a3c414ae55750a` established absence of a CLI directory in this image. Inventory `ev_85db8091c579adff188648e239021de194d47ea4bd48742cdd4b356ae77c47bb` identified one PE root. Native Ghidra overview `ev_97c9c956b50c9f2a28e6c3ff3b84537d12b035690a627452354795ccb5bc6ea0` reports 1,258 recovered procedures and 1,048 defined strings after default auto-analysis. These are provider observations, not original source counts.

| Entry in this exact image | REA dossier | Static observation and boundary |
| --- | --- | --- |
| `0x18001f7a0`, `SketchGrid::setMinorIntervalForPerspectiveCamera` | `ev_fc7efe790b369c0c46248961f81fe882d2f28e0d57b610f5f6e8e6bf176faed6` | Checks the snap/grid manager, obtains a camera and conditionally calls `updateCurrentSnapIncrement` using a geometric point. The numerical projection enum and full increment policy need further analysis/runtime correlation. |
| `0x180013a70`, `ProfileAdorner::color` setter | `ev_1333efd3b8a5285c8157cf77305abf58c8eefb1465ce55d121aacc74b03734a9` | Compares the existing color to the argument, assigns and reactivates the adorner only on change. The actual selected-theme color is not supplied by this setter. |
| `0x18000a0a0`, `DimensionAdorner::onApplyAdornment` | `ev_7f7fc0ac1f13c6d618c4abc9f3a67693333063fda0ee2da4b9c9559e07038047` | Distinct branches call selected, highlighted, and selected-plus-highlighted color accessors. Recovered code sets a line-style weight of 1.5 and configures depth/occlusion effects. Device-pixel width and final colors require runtime evidence. |
| `0x1800208a0`, `SketchGrid::updateSketchGrid` | `ev_8d9967eb0e2f20fb786c2581f93711f0f7e11c62d577ecf41809e020eb384d17` | Looks up the active session/adorner, obtains sketch-axis colors and scene bounds, and can enlarge the bounds for a transformed input point. Indirect calls and inferred types limit reconstruction. |

Native symbol search used `ev_b7c45bb8381408b839d20ed7fa9b12d832eacd9d3d24ea671db5055238c36b01`. This module covers adornment/grid behavior; it is not the complete sketch tool or constraint solver implementation.

The image references `NaSketchUI10.pdb`, GUID `59a500b7-e950-496f-96df-7955dc8f1ac2`, age 1. Ghidra reports its load state unknown. Original names/types can be missing. A bounded package search found no `.pdb` files; it did not search symbol services.

## Modeling execution

Target `Applications/Fusion/NaFusion10.dll`, 52,822,080 bytes, SHA-256 `14139e2502f1ccfec01af3857edf3bb5f934a323587decba812809d7ad6cbf24`. Ordinary LLVM export inspection located `ExtrudeRequest::onExecute`, `onValidateInput`, `createToolBody` and `ParametricExtrudeRequest::onExecute`. These names are entry seeds, not decompiled behavior.

The initial native request hit the MCP client's default 60-second deadline; its local driver was corrected to pass timeout options using the installed client's two-argument API. The subsequent import hit REA's separate default 330000 ms provider startup deadline. A new owned session used a process-local 1200000 ms startup deadline with all default analyzers enabled; that attempt also returned `provider_timeout` before analysis completed. No successful modeling-body Evidence ID is claimed. Failure output and retry provenance remain in the external reference folder. Cleanup closed the target and owned client; no modeling Java process was left running.

A bounded **ordinary LLVM disassembly/import-table observation** covers the exported `ExtrudeRequest::onExecute` and its adjacent post-execute method. This supplementary observation has no REA Evidence ID. Its exact source image has PE image base `0x180000000`, `.text` RVA `0x1000` and file offset `0x400`, established by LLVM headers. The retained span is `0x1817b6fe0` through before `0x1817b73e0`; it includes more than one function and must not be presented as one recovered function body.

External `modeling-extended-session/extrude-execute-llvm-span.txt`, `modeling-imports-llvm.txt` and `extrude-execute-import-map.json` support these limited statements:

- At `0x1817b700f`, the import slot resolves to `BodyModificationRequest::onExecute` in `NsBody10.dll`.
- At `0x1817b7020`, the import slot resolves to `OccurrencePath::targetComponent`. A later null test selects an error-building path. Exact UI error text and runtime reachability have not been established.
- At `0x1817b7297`, the import slot resolves to an `ASMScopedLock` constructor; a corresponding destructor call occurs at `0x1817b7375`. A lock's presence does not establish Fusion's complete transaction or threading policy.
- There are unresolved virtual calls through slot `0x18`, status checks through imported `Status::isOk`, and a conditional alternate path. Exact virtual method semantics and mode/flag meanings remain unknown.
- At `0x1817b734c`, the import slot resolves to `Request::isPreview`. The subsequent instructions transfer the Boolean result to the second argument of the direct `ToolCreationRequestUtil::consumeToolBody` call at `0x1817b735e`. This establishes explicit preview-state handling at that boundary; it does not prove preview rollback or solver behavior.
- In the adjacent `onPostExecute` method, the imported `isDirectModeling` predicate gates a direct call to the exported `ExtrudeRequest::createFeatures`, followed by the base `BodyModificationRequest::onPostExecute` boundary.

Import slots were matched by their PE import-address-table position and 8-byte x64 entries. Symbol-plus-offset comments from LLVM are not treated as authenticated function boundaries or semantic names. Ghidra function dossiers are still needed to corroborate inferred interfaces and broader control flow. Helper-process thread observations showed initial entry-point disassembly, then constant-propagation analysis later in the extended attempt. A future profile should address that measured analysis cost and retain its changed coverage explicitly, rather than repeat the same timed-out import unchanged.

## Sketch constraint and dimension model requests

Target `Applications/DirectModeling/NaSketch10.dll`, 4,339,264 bytes, x86-64 PE, SHA-256 `59e3b67fecfb0db0924f656f36e2e696431c0089290ab96a4344b7ff49ac4dd6`. A separate default Ghidra analysis completed in approximately 184 seconds on 2026-10-19. Complete evidence, small projections and an exported bundle are retained under external `sketch-model-study/extended`; `sketch-model-study/report.md` records call sites and remaining uncertainties. That owned session was closed successfully.

| Entry in this exact image | REA dossier | Static observation and boundary |
| --- | --- | --- |
| `0x1801faba0`, `SketchConstraintsSolveRequest::onExecute` | `ev_7eec01419e9db485de5a37d5a6b20f45010989f2a80f73180c6e8d1f06555be7` | A 93-byte wrapper calls base request execution, then delegates to `doSketchConstraintSolving` with geometry references and the shared status. Flag meanings, solver algorithm, tolerances and rollback policy are unknown. |
| `0x1801ce5c0`, `DeleteSketchDimensionRequest::onExecute` | `ev_263edb26316bfc2e602293792dd2b1cc8821c931358a6a38684b99882162b6f3` | A 1,253-byte body guards the dimension reference, resolves its owning sketch, calls `removeDimension`, examines referenced geometry and constraints, then clears the request's target reference. Six indirect call sites remain unresolved; surface-specific cleanup semantics are unknown. |

This supports an independent model requirement: dimensions need explicit sketch ownership and geometry dependency edges. Fission 180's existing last-good-document rule calls for an awaited atomic edit when deleting a dimension. That transaction policy is a Fission 180 design choice; these two bodies do not establish Fusion's complete transaction mechanism. A driving-dimension delete/undo/redo runtime comparison remains pending.

## Interpretation and next evidence

Decompiler output is recovered pseudocode. Resolved static calls establish code relationships, not execution in a specific user scenario. Unresolved indirect calls remain unknown. These investigations do not recover the original source tree, establish complete geometry tolerances, or prove product parity.

Independent implementation work should use a written behavior contract, followed by Rust tests and a matching Fusion runtime scenario. Grid bounds/camera handling, dimension selection states, command availability, expression editing, and extrusion execution are distinct contracts. Track their progress in [parity.md](parity.md).
