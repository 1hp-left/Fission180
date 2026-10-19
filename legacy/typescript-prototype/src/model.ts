import { exportSTEP, makeCompound, measureVolume, type Drawing, type Shape3D } from 'replicad';
import { type CadDocument, type SketchFeature, parseDocument, resolveScalar } from './document.js';
import { initializeKernel } from './kernel.js';
import { createProfile, LENGTH_TOLERANCE } from './profiles.js';

export class FeatureError extends Error {
  constructor(public readonly featureId: string, cause: unknown) {
    super(`Feature "${featureId}": ${cause instanceof Error ? cause.message : String(cause)}`, { cause });
    this.name = 'FeatureError';
  }
}

export interface BodySummary {
  id: string;
  volume: number;
  solidCount: number;
  bounds: { min: number[]; max: number[]; size: number[] };
}

export interface ModelSummary {
  name: string;
  units: 'mm';
  volumeUnits: 'mm³';
  featureCount: number;
  suppressedCount: number;
  bodies: BodySummary[];
}

export class BuiltModel {
  #disposed = false;
  constructor(private readonly document: CadDocument, private readonly bodies: Map<string, Shape3D>) {}

  private assertAlive(): void {
    if (this.#disposed) throw new Error('Model has been disposed');
  }

  summary(): ModelSummary {
    this.assertAlive();
    return {
      name: this.document.name, units: 'mm', volumeUnits: 'mm³',
      featureCount: this.document.features.length,
      suppressedCount: this.document.features.filter(f => f.suppressed).length,
      bodies: [...this.bodies].map(([id, shape]) => {
        const box = shape.boundingBox;
        const [min, max] = box.bounds;
        box.delete();
        const solids = shape.solids;
        const solidCount = solids.length;
        solids.forEach(s => s.delete());
        return { id, volume: measureVolume(shape), solidCount, bounds: { min, max, size: max.map((n, i) => n - min[i]!) } };
      }),
    };
  }

  exportSTEP(): Blob {
    this.assertAlive();
    if (!this.bodies.size) throw new Error('No solid bodies to export');
    return exportSTEP([...this.bodies].map(([name, shape]) => ({ name, shape })), { unit: 'MM', modelUnit: 'MM' });
  }

  exportSTL(): Blob {
    this.assertAlive();
    if (!this.bodies.size) throw new Error('No solid bodies to export');
    // Replicad's compound builder consumes its inputs. Preserve live bodies.
    const compound = makeCompound([...this.bodies.values()].map(shape => shape.clone()));
    try { return compound.blobSTL({ binary: true, tolerance: 0.05, angularTolerance: 0.1 }); }
    finally { compound.delete(); }
  }

  sketchSVG(id: string): string {
    this.assertAlive();
    const sketch = this.document.features.find(f => f.id === id && f.type === 'sketch' && !f.suppressed);
    if (!sketch || sketch.type !== 'sketch') throw new Error(`Active sketch "${id}" does not exist`);
    return createProfile(sketch.profile, this.document.parameters).toSVG(2);
  }

  dispose(): void {
    if (this.#disposed) return;
    this.bodies.forEach(shape => shape.delete());
    this.bodies.clear();
    this.#disposed = true;
  }
}

/** Replays ordered features. A failed build releases the bodies it constructed. */
export async function buildModel(input: unknown): Promise<BuiltModel> {
  const document = parseDocument(input);
  const oc = await initializeKernel();
  const sketches = new Map<string, { feature: SketchFeature; drawing: Drawing }>();
  const bodies = new Map<string, Shape3D>();
  try {
    for (const feature of document.features) {
      if (feature.suppressed) continue;
      try {
        if (feature.type === 'sketch') {
          const drawing = createProfile(feature.profile, document.parameters);
          sketches.set(feature.id, { feature, drawing });
          continue;
        }
        const sketch = sketches.get(feature.sketch);
        if (!sketch) throw new Error(`Sketch "${feature.sketch}" must be active and appear earlier in the timeline`);
        const distance = resolveScalar(feature.distance, document.parameters);
        if (Math.abs(distance) <= LENGTH_TOLERANCE) throw new Error('Extrusion distance must be nonzero');
        const target = bodies.get(feature.body);
        if (feature.operation === 'new' && target) throw new Error(`Body "${feature.body}" already exists`);
        if (feature.operation !== 'new' && !target) throw new Error(`Target body "${feature.body}" does not exist`);
        const origin = sketch.feature.origin.map(n => resolveScalar(n, document.parameters)) as [number, number, number];
        const tool = sketch.drawing.sketchOnPlane(sketch.feature.plane, origin).extrude(distance).asShape3D();
        let result: Shape3D | undefined;
        try {
          if (feature.operation === 'new') result = tool;
          else if (feature.operation === 'join') result = target!.fuse(tool);
          else if (feature.operation === 'cut') result = target!.cut(tool);
          else result = target!.intersect(tool);
          const analyzer = new oc.BRepCheck_Analyzer(result.wrapped, true, false, false);
          try {
            if (!analyzer.IsValid()) throw new Error('Operation produced invalid solid topology');
          } finally { analyzer.delete(); }
          const volume = measureVolume(result);
          const solids = result.solids;
          const hasSolids = solids.length > 0;
          solids.forEach(s => s.delete());
          if (!hasSolids || !Number.isFinite(volume) || volume <= 0) throw new Error('Operation produced an empty or non-solid result');
          bodies.set(feature.body, result);
          target?.delete();
        } catch (error) {
          if (result && result !== tool) result.delete();
          throw error;
        } finally {
          if (bodies.get(feature.body) !== tool) tool.delete();
        }
      } catch (error) { throw new FeatureError(feature.id, error); }
    }
    return new BuiltModel(document, bodies);
  } catch (error) {
    bodies.forEach(shape => shape.delete());
    throw error;
  }
}
