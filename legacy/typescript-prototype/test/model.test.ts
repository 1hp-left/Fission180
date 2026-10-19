import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { importSTEP, measureVolume } from 'replicad';
import { buildModel, ModelSession, parseDocument, type CadDocument } from '../src/index.js';

const basic = () => parseDocument({
  format: 'openfusion', version: 1, name: 'Test part', units: 'mm', parameters: { width: 40, depth: 5 },
  features: [
    { id: 'profile', type: 'sketch', profile: { type: 'rectangle', width: { parameter: 'width' }, height: 30 } },
    { id: 'pad', type: 'extrude', sketch: 'profile', distance: { parameter: 'depth' }, body: 'body' },
  ],
});
const near = (actual: number, expected: number, tolerance = 1e-5) => assert.ok(Math.abs(actual - expected) < tolerance, `${actual} != ${expected}`);
const volume = (session: ModelSession) => session.model.summary().bodies[0]!.volume;

test('rectangular extrusion has analytic volume and bounding box', async () => {
  const model = await buildModel(basic());
  try {
    const body = model.summary().bodies[0]!;
    near(body.volume, 6000); assert.equal(body.solidCount, 1);
    body.bounds.size.forEach((n, i) => near(n, [40, 30, 5][i]!));
  } finally { model.dispose(); }
});

test('circle cuts, joins, and intersects are real solid operations', async () => {
  for (const operation of ['cut', 'join', 'intersect'] as const) {
    const document = basic();
    document.features.push(
      { id: 'circle', type: 'sketch', plane: 'XY', origin: [0, 0, 0], suppressed: false, profile: { type: 'circle', radius: 5, center: [0, 0] } },
      { id: 'boolean', type: 'extrude', sketch: 'circle', distance: 10, operation, body: 'body', suppressed: false },
    );
    const model = await buildModel(document);
    try { near(model.summary().bodies[0]!.volume, operation === 'cut' ? 6000 - 125 * Math.PI : operation === 'join' ? 6000 + 125 * Math.PI : 125 * Math.PI); }
    finally { model.dispose(); }
  }
});

test('polygon profiles close automatically and support concavity and winding', async () => {
  for (const reverse of [false, true]) {
    const document = basic();
    const sketch = document.features[0]!;
    assert.equal(sketch.type, 'sketch');
    if (sketch.type !== 'sketch') throw new Error();
    const points: [number, number][] = [[0, 0], [10, 0], [10, 5], [5, 5], [5, 10], [0, 10]];
    sketch.profile = { type: 'polygon', points: reverse ? points.reverse() : points };
    const model = await buildModel(document);
    try { near(model.summary().bodies[0]!.volume, 375); } finally { model.dispose(); }
  }
});

test('bad sketch geometry is rejected with feature attribution', async () => {
  const invalid: [number, number][][] = [
    [[0, 0], [10, 10], [0, 10], [10, 0]],
    [[0, 0], [10, 0], [5, 0], [5, 10]],
    [[0, 0], [10, 0], [10, 10], [0, 0]],
    [[0, 0], [10, 0], [20, 0]],
  ];
  for (const points of invalid) {
    const document = basic();
    const sketch = document.features[0]!;
    if (sketch.type === 'sketch') sketch.profile = { type: 'polygon', points };
    await assert.rejects(buildModel(document), /Feature "profile": Polygon/);
  }
});

test('parameter edits replay the timeline; undo and redo restore geometry', async () => {
  const session = await ModelSession.create(basic());
  try {
    await session.setParameter('width', 80); near(volume(session), 12000);
    assert.equal(await session.undo(), true); near(volume(session), 6000);
    assert.equal(await session.redo(), true); near(volume(session), 12000);
    await session.undo(); await session.setParameter('depth', 10);
    near(volume(session), 12000); assert.equal(session.canRedo, false);
    const copy = session.document; copy.parameters.width = 999;
    assert.equal(session.document.parameters.width, 40);
  } finally { session.dispose(); }
});

test('failed edits preserve document, valid geometry, and undo/redo history', async () => {
  const session = await ModelSession.create(basic());
  try {
    await session.setParameter('width', 80); await session.undo();
    const original = session.document;
    await assert.rejects(session.setParameter('width', -1), /Width must exceed/);
    assert.deepEqual(session.document, original); near(volume(session), 6000);
    assert.equal(session.canUndo, false); assert.equal(session.canRedo, true);
    await assert.rejects(session.suppressFeature('profile', true), /must be active/);
    assert.deepEqual(session.document, original); near(volume(session), 6000);
    await assert.rejects(session.setParameter('depth', 0), /distance must be nonzero/);
    await assert.rejects(session.setParameter('missing', 10), /Unknown parameter/);
    await session.redo(); near(volume(session), 12000);
  } finally { session.dispose(); }
});

test('feature suppression and deletion rebuild without stale bodies', async () => {
  const session = await ModelSession.create(basic());
  try {
    await session.suppressFeature('pad', true); assert.deepEqual(session.model.summary().bodies, []);
    await session.undo(); near(volume(session), 6000);
    const candidate = session.document; candidate.features = [];
    await session.replace(candidate); assert.deepEqual(session.model.summary().bodies, []);
    await session.undo(); near(volume(session), 6000);
  } finally { session.dispose(); }
});

test('plane origins, rotations, and negative extrusion affect bounds correctly', async () => {
  for (const plane of ['XY', 'XZ', 'YZ'] as const) {
    const document = basic();
    const sketch = document.features[0]!;
    if (sketch.type === 'sketch') { sketch.plane = plane; sketch.origin = [100, 200, 300]; }
    document.parameters.depth = -5;
    const model = await buildModel(document);
    try {
      const body = model.summary().bodies[0]!; near(body.volume, 6000);
      assert.deepEqual(body.bounds.size.map(n => Math.round(n)).sort((a, b) => a - b), [5, 30, 40]);
      const normalAxis = plane === 'XY' ? 2 : plane === 'XZ' ? 1 : 0;
      // Replicad's XZ plane points along -Y; the others point along +Z/+X.
      const expectedMin = [100, 200, 300][normalAxis]! + (plane === 'XZ' ? 0 : -5);
      near(body.bounds.min[normalAxis]!, expectedMin);
    } finally { model.dispose(); }
  }
  const document = basic();
  const sketch = document.features[0]!;
  if (sketch.type === 'sketch' && sketch.profile.type === 'rectangle') sketch.profile.rotation = 90;
  const model = await buildModel(document);
  try { const size = model.summary().bodies[0]!.bounds.size; near(size[0]!, 30); near(size[1]!, 40); }
  finally { model.dispose(); }
});

test('schema rejects unknown fields, duplicate IDs, nonfinite values, and future formats', () => {
  assert.throws(() => parseDocument({ ...basic(), version: 2 }));
  assert.throws(() => parseDocument({ ...basic(), script: 'untrusted' }));
  const duplicate = basic(); duplicate.features.push(duplicate.features[0]!);
  assert.throws(() => parseDocument(duplicate), /Duplicate feature id/);
  assert.throws(() => parseDocument({ ...basic(), parameters: { width: Infinity } }));
  assert.throws(() => parseDocument({ ...basic(), units: 'inch' }));
});

test('invalid dependencies, missing targets, empty cuts, and missing parameters fail', async () => {
  const missing = basic(); delete missing.parameters.width;
  await assert.rejects(buildModel(missing), /Unknown parameter "width"/);
  const ordered = basic(); ordered.features.reverse();
  await assert.rejects(buildModel(ordered), /appear earlier/);
  const target = basic(); const pad = target.features[1]!;
  if (pad.type === 'extrude') pad.operation = 'cut';
  await assert.rejects(buildModel(target), /Target body/);
  const empty = basic();
  empty.features.push({ id: 'remove', type: 'extrude', sketch: 'profile', distance: 5, operation: 'cut', body: 'body', suppressed: false });
  await assert.rejects(buildModel(empty), /empty or non-solid/);
});

test('mounting plate volume matches analytic result and STEP round trip', async () => {
  const document: unknown = JSON.parse(await readFile(new URL('../examples/mounting-plate.of.json', import.meta.url), 'utf8'));
  const model = await buildModel(document);
  try {
    const expected = 80 * 50 * 6 - 4 * Math.PI * 3.5 ** 2 * 6 + Math.PI * 12 ** 2 * 4 - Math.PI * 4 ** 2 * 10;
    near(model.summary().bodies[0]!.volume, expected);
    const step = model.exportSTEP();
    assert.match(await step.text(), /ISO-10303-21/);
    const imported = (await importSTEP(step)).asShape3D();
    try { near(measureVolume(imported), expected, 1e-4); } finally { imported.delete(); }
    const stl = await model.exportSTL().arrayBuffer();
    const count = new DataView(stl).getUint32(80, true);
    assert.ok(count > 12); assert.equal(stl.byteLength, 84 + 50 * count);
    near(model.summary().bodies[0]!.volume, expected);
    assert.match(await model.exportSTEP().text(), /ISO-10303-21/);
    assert.ok((await model.exportSTL().arrayBuffer()).byteLength > 84);
    near(model.summary().bodies[0]!.volume, expected);
    assert.match(model.sketchSVG('base-profile'), /<svg/);
  } finally { model.dispose(); }
});

test('multiple bodies export to STEP without losing volume', async () => {
  const document = basic();
  document.features.push(
    { id: 'other-profile', type: 'sketch', plane: 'XY', origin: [100, 0, 0], suppressed: false, profile: { type: 'circle', radius: 2, center: [0, 0] } },
    { id: 'other-pad', type: 'extrude', sketch: 'other-profile', distance: 10, body: 'other', operation: 'new', suppressed: false },
  );
  const model = await buildModel(document);
  try {
    assert.equal(model.summary().bodies.length, 2);
    const imported = (await importSTEP(model.exportSTEP())).asShape3D();
    try { near(measureVolume(imported), 6000 + 40 * Math.PI); } finally { imported.delete(); }
  } finally { model.dispose(); }
});

test('empty models cannot export and disposed models reject use', async () => {
  const model = await buildModel({ ...basic(), features: [] });
  assert.throws(() => model.exportSTEP(), /No solid bodies/);
  assert.throws(() => model.exportSTL(), /No solid bodies/);
  model.dispose(); model.dispose();
  assert.throws(() => model.summary(), /disposed/);
});
