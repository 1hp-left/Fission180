# Fission 180 development

Formerly OpenFusion. Active Cargo packages use `fission180-*` and the desktop executable is `fission180.exe`. Keep the existing source/evidence paths and version-1 `.ofrust` format (`openfusion-native`) compatible. Historical audits retain their original identity. The TypeScript prototype was removed at the user's request; its old source exists only in Git history.

Build an independent open-source CAD application. The user requires a near pixel-for-pixel Fusion desktop interface and a native Rust implementation. Sketching and solid modeling remain the first functional scope. Do not substitute a headless CLI or a generic redesigned interface for that goal.

## Evidence and reconstruction

- Use the installed `reverse-engineer-anything` skill for Fusion artifact/runtime investigations. Use ordinary source tools for Fission 180 development.
- Separate observed Fusion behavior, implementation choices, and unknowns. Maintain `docs/investigation.md` as the investigation index.
- Fusion has been reinstalled and logged in. The observed reference is build 2705.1.30 with the dark blue theme at 125% Windows scaling. Do not substitute old Autodesk caches for the selected executable.
- REA 6.7.0 CLI is installed. Codex MCP registration and native providers were unavailable at bootstrap. Use supported CLI inspection where possible; do not silently change global client configuration or install native engines.
- The user subsequently approved portable Ghidra 12.1.4 and Temurin Java 21 under `C:\Users\Vnm4\Downloads\openfusion-reference\tools`. They work through REA with process-local settings; global client configuration remains unchanged. Reuse the retained native Evidence indexed in `docs/rea-native.md`. Large-module analysis needs a longer process-local `REA_GHIDRA_STARTUP_TIMEOUT_MS` than the default 330000 ms.
- Keep target binaries and extracted Autodesk assets outside this source tree. Implement behavior independently and retain source provenance for findings.

## Modeling

- Retain OpenCascade as the default solid backend while progressively implementing an independently written native Rust B-rep kernel behind the explicit backend boundary. Unsupported Rust operations must fail explicitly; never silently fall back to OCCT. Meshes are for display/export and never authoritative solid geometry.
- Document dimensions are millimetres. Version and validate persisted data.
- Preserve the last good document and geometry on failed edits. Keep feature dependencies explicit.
- Await session edits. Dispose owned kernel-backed shapes on success and failure paths.
- Keep geometry logic independent of the future GUI.
- Run `cargo fmt --all -- --check`, `cargo check --workspace`, and `cargo test --workspace` for Rust changes. Add meaningful analytic or interoperability checks for new operations.
- Use the observed Fusion layout and compare actual screenshots. Preserve system DPI scaling. Do not claim pixel perfection or feature parity without evidence.
- Rust owns the active app. Do not introduce a TypeScript/browser runtime dependency.

## Current layout

- `crates/openfusion-core/`: Rust document validation, native OpenCascade modeling, replay and history.
- `crates/openfusion-desktop/`: Rust/egui/wgpu interface, original vector icons, viewport and geometry worker.
- `examples/`: native `.ofrust` documents.
- `docs/fidelity.md`: visual target and gaps.
