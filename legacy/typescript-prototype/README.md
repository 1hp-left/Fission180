# OpenFusion

An independent open-source CAD project, starting with parametric sketch profiles and solid modeling. The aim is a usable alternative to Autodesk Fusion. This first milestone is a **modeling library and command-line tool**, with real boundary-representation solids powered by OpenCascade through [Replicad](https://replicad.pages.dev/docs/use-as-a-library/).

There is no interactive CAD window yet. Fusion has not yet been inspected: its installation was deleted before this workspace was created. The implementation below is independent engineering; no claim of Fusion parity is established.

## Run

Requires Node.js 22 or newer and npm. Development was verified on Windows with Node 24.22.0.

```powershell
npm ci
npm run check
npm test

# Inspect and recompute the example part.
npm run cad -- inspect examples/mounting-plate.of.json

# Export a solid model for another CAD application.
npm run cad -- build examples/mounting-plate.of.json --out output/mounting-plate.step

# Export a printable mesh.
npm run cad -- build examples/mounting-plate.of.json --out output/mounting-plate.stl

# Edit a driving dimension, rebuild, and save the editable model.
npm run cad -- build examples/mounting-plate.of.json --set width=100 --out output/wide-plate.step --save output/wide-plate.of.json

# Inspect a sketch as SVG in its local plane coordinates.
npm run cad -- build examples/mounting-plate.of.json --out output/base-profile.svg --sketch base-profile
```

Exports refuse to overwrite existing files unless `--force` is supplied. All geometry computation runs locally; the installed dependencies include the WebAssembly kernel. The CLI prints JSON with body volumes, bounding boxes, and output paths.

## What works

- Rectangle profiles with width, height, center, and rotation; circles with radius and center; closed polygons, including concave polygons.
- Sketches on XY, XZ, and YZ planes with a movable origin.
- Literal dimensions or references to named numeric parameters, in millimetres. Rectangle and circle centers default to the sketch origin.
- Positive and negative extrusions: new body, join, cut, and intersect.
- Ordered feature replay with dependency errors tied to the feature that failed.
- Feature suppression, parameter edits, and atomic recomputation in `ModelSession`. A failed edit preserves the previous document, geometry, and undo/redo stacks.
- Undo/redo, bounded to 100 previous edits, in the library.
- Kernel topology checks and rejection of empty/non-solid results.
- STEP assembly export, binary STL export, and individual sketch SVG export.
- A versioned JSON format with strict runtime validation. Documents contain data, not executable scripts.

The example is an 80 × 50 × 6 mm mounting plate with four 7 mm holes, a 24 mm diameter raised boss, and an 8 mm center bore. Its modeled volume is compared against an analytic calculation and checked again after a STEP export/import round trip.

## Library example

```typescript
import { readFile } from 'node:fs/promises';
import { ModelSession } from './src/index.js';

const document = JSON.parse(await readFile('examples/mounting-plate.of.json', 'utf8'));
const session = await ModelSession.create(document);
try {
  await session.setParameter('width', 100);
  console.log(session.model.summary());
  await session.undo();
  await session.redo();
  // session.document is a defensive copy suitable for JSON persistence.
  // session.model.exportSTEP() returns a Blob.
} finally {
  session.dispose();
}
```

Await each edit before making another. Builds currently execute in the Node process. A future interactive frontend should run geometry computation in a worker. Dispose models and sessions when finished to release their solid geometry.

## Model format

See `examples/mounting-plate.of.json` and `src/document.ts` for the complete current format. Features must appear after their dependencies. Feature IDs are unique. An extrusion's `body` names either the new body or an existing target body.

```json
{
  "format": "openfusion",
  "version": 1,
  "name": "Block",
  "units": "mm",
  "parameters": { "width": 40 },
  "features": [
    {
      "id": "profile",
      "type": "sketch",
      "profile": {
        "type": "rectangle",
        "width": { "parameter": "width" },
        "height": 30
      }
    },
    {
      "id": "pad",
      "type": "extrude",
      "sketch": "profile",
      "distance": 5,
      "body": "block"
    }
  ]
}
```

Polygons close automatically; do not repeat the first point at the end. Crossings, touching nonadjacent segments, and overlapping adjacent edges are rejected. Dimensions must exceed 0.00001 mm in magnitude where applicable; widths, heights, and radii must be positive. Parameter and literal magnitudes are limited to 1,000,000. The plane normals follow Replicad: XY → +Z, XZ → −Y, YZ → +X. Sketch `origin` is a world coordinate; profile coordinates are local to the plane.

## Next milestones and limits

1. Observe Fusion's create-sketch → dimension → extrude → edit workflow after reinstall, recording evidence and unresolved questions in `docs/investigation.md`.
2. Build an interactive 2D sketch editor and 3D viewport over the tested model/session API.
3. Add a geometric constraint solver with degrees-of-freedom reporting. Current dimensioned primitives are not a general constraint solver.
4. Extend features: revolve, fillet, chamfer, shell, patterns, assemblies, and stable face/edge references.
5. Expand interoperability, then evaluate manufacturing and simulation as separate projects.

This milestone has no freehand line/arc editor, dimensional expressions, face-based sketching, general constraints, GUI, Fusion document importer, CAM, rendering environment, or simulation. Parameters are independent: for example, widening the example plate does not reposition its holes. Boolean operations that remove every solid fail instead of silently deleting the body. Disconnected solids may share a body after a Boolean operation; `solidCount` reports this.

## Source and licenses

OpenFusion's original code is MIT licensed. Autodesk Fusion names identify the comparison target; this is an independent project. See `THIRD_PARTY_NOTICES.md` for the kernel's separate license and sources. Dependency versions are recorded in `package-lock.json`.

`npm test` checks geometry, failure behavior, history, and STEP/STL exports. `npm run check` checks TypeScript types.
