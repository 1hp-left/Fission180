# Third-party components

The original Fission 180 source is licensed under MIT; third-party components retain their own licenses.

| Component | Role | License | Source |
| --- | --- | --- | --- |
| cadrum 0.9.0 | Rust bindings to native OpenCascade | MIT | https://github.com/lzpel/cadrum |
| Open CASCADE Technology 8.0.1 | Native solid modeling kernel, statically linked | LGPL 2.1 with the OCCT additional exception | https://occt3d.com/open-cascade-technology/index.html |
| egui / eframe / egui-wgpu 0.37.0 | Native UI and GPU integration | MIT OR Apache-2.0 | https://github.com/emilk/egui |
| wgpu | GPU abstraction | MIT OR Apache-2.0 | https://github.com/gfx-rs/wgpu |
| serde / serde_json | Document serialization | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| rfd | Native file dialogs | MIT | https://github.com/PolyMeilex/rfd |
| image | QA screenshot encoding | MIT OR Apache-2.0 | https://github.com/image-rs/image |

The Rust binding downloads a prebuilt native OCCT archive during the first build and links it statically. Its source, licenses, and build recipe are available from the binding repository and its release artifacts. Preserve the relevant license texts, notices, source availability, and relinking materials when preparing redistributable packages. No public binary distribution has been published by this task.

The application reads Segoe UI and Segoe Symbols from the local Windows installation and falls back to egui's bundled fonts. Microsoft font files are not redistributed. Autodesk binaries and assets are not included in this source tree; toolbar icons are original vector drawings.

The archived TypeScript experiment uses Replicad (MIT), replicad-opencascadejs (LGPL-2.1-only package declaration), and Zod (MIT). Those npm dependencies and the WebAssembly kernel are not used by the native Rust app.

For transitive and development dependencies, consult the locked package manifests and distributed license files. This list identifies major runtime components and is not a complete license inventory for a future application distribution.
