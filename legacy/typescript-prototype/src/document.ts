import { z } from 'zod';

const identifier = z.string().regex(/^[A-Za-z][A-Za-z0-9_-]{0,63}$/);
const numeric = z.number().finite().min(-1_000_000).max(1_000_000);
export const scalarSchema = z.union([numeric, z.strictObject({ parameter: identifier })]);
const point2 = z.tuple([scalarSchema, scalarSchema]);
const point3 = z.tuple([scalarSchema, scalarSchema, scalarSchema]);
const profileSchema = z.discriminatedUnion('type', [
  z.strictObject({
    type: z.literal('rectangle'), width: scalarSchema, height: scalarSchema,
    center: point2.default([0, 0]), rotation: scalarSchema.default(0),
  }),
  z.strictObject({ type: z.literal('circle'), radius: scalarSchema, center: point2.default([0, 0]) }),
  z.strictObject({ type: z.literal('polygon'), points: z.array(point2).min(3).max(1000) }),
]);
const common = { id: identifier, name: z.string().min(1).max(128).optional(), suppressed: z.boolean().default(false) };
export const featureSchema = z.discriminatedUnion('type', [
  z.strictObject({
    ...common, type: z.literal('sketch'), plane: z.enum(['XY', 'XZ', 'YZ']).default('XY'),
    origin: point3.default([0, 0, 0]), profile: profileSchema,
  }),
  z.strictObject({
    ...common, type: z.literal('extrude'), sketch: identifier, distance: scalarSchema,
    operation: z.enum(['new', 'join', 'cut', 'intersect']).default('new'), body: identifier,
  }),
]);

export const documentSchema = z.strictObject({
  format: z.literal('openfusion'), version: z.literal(1),
  name: z.string().min(1).max(128), units: z.literal('mm'),
  parameters: z.record(identifier, numeric).default({}),
  features: z.array(featureSchema).max(1000),
}).superRefine((document, ctx) => {
  const ids = new Set<string>();
  document.features.forEach((feature, index) => {
    if (ids.has(feature.id)) ctx.addIssue({ code: 'custom', path: ['features', index, 'id'], message: `Duplicate feature id: ${feature.id}` });
    ids.add(feature.id);
  });
});

export type CadDocument = z.infer<typeof documentSchema>;
export type Feature = CadDocument['features'][number];
export type SketchFeature = Extract<Feature, { type: 'sketch' }>;
export type Profile = SketchFeature['profile'];
export type Scalar = z.infer<typeof scalarSchema>;

/** Parses data only: expressions, scripts, and unknown properties are rejected. */
export function parseDocument(value: unknown): CadDocument {
  return documentSchema.parse(value);
}

export function resolveScalar(value: Scalar, parameters: CadDocument['parameters']): number {
  if (typeof value === 'number') return value;
  if (!Object.hasOwn(parameters, value.parameter)) throw new Error(`Unknown parameter "${value.parameter}"`);
  return parameters[value.parameter]!;
}
