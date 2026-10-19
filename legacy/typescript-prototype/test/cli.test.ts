import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = fileURLToPath(new URL('../', import.meta.url));
const cli = (...args: string[]) => spawnSync(process.execPath, ['--import', 'tsx', 'src/cli.ts', ...args], {
  cwd: root, encoding: 'utf8', timeout: 30_000,
});

async function removeTestDirectory(path: string): Promise<void> {
  const absolute = resolve(path);
  assert.equal(dirname(absolute), resolve(tmpdir()));
  assert.ok(basename(absolute).startsWith('openfusion-cli-'));
  await rm(absolute, { recursive: true, force: true });
}

test('CLI exports, persists parameter changes, and refuses accidental overwrite', async () => {
  const temporary = await mkdtemp(join(tmpdir(), 'openfusion-cli-'));
  try {
    const input = 'examples/mounting-plate.of.json';
    const output = join(temporary, 'wide.step');
    const saved = join(temporary, 'wide.of.json');
    const build = cli('build', input, '--set', 'width=100', '--out', output, '--save', saved);
    assert.equal(build.status, 0, build.stderr);
    const report = JSON.parse(build.stdout);
    assert.equal(report.bodies[0].bounds.size[0], 100);
    assert.match(await readFile(output, 'utf8'), /ISO-10303-21/);
    assert.equal(JSON.parse(await readFile(saved, 'utf8')).parameters.width, 100);
    const original = await readFile(output);
    const overwrite = cli('build', input, '--out', output);
    assert.equal(overwrite.status, 1); assert.match(overwrite.stderr, /EEXIST/);
    assert.deepEqual(await readFile(output), original);
    const force = cli('build', input, '--out', output, '--force');
    assert.equal(force.status, 0, force.stderr);
    assert.equal(JSON.parse(force.stdout).bodies[0].bounds.size[0], 80);
    const stlOutput = join(temporary, 'plate.stl');
    const stl = cli('build', input, '--out', stlOutput);
    assert.equal(stl.status, 0, stl.stderr);
    assert.equal(JSON.parse(stl.stdout).bodies[0].solidCount, 1);
    const bytes = await readFile(stlOutput);
    assert.equal(bytes.length, 84 + 50 * bytes.readUInt32LE(80));
    const svgOutput = join(temporary, 'profile.svg');
    const svg = cli('build', input, '--out', svgOutput, '--sketch', 'base-profile');
    assert.equal(svg.status, 0, svg.stderr);
    assert.match(await readFile(svgOutput, 'utf8'), /<svg/);
  } finally { await removeTestDirectory(temporary); }
});

test('CLI reports malformed inputs and options without writing geometry', async () => {
  const temporary = await mkdtemp(join(tmpdir(), 'openfusion-cli-'));
  try {
    const input = join(temporary, 'bad.json');
    await writeFile(input, '{ invalid');
    assert.equal(cli('inspect', input).status, 1);
    const example = 'examples/mounting-plate.of.json';
    const missing = cli('inspect', example, '--set', 'unknown=1');
    assert.equal(missing.status, 1); assert.match(missing.stderr, /Unknown parameter/);
    const bad = cli('build', example, '--out', join(temporary, 'part.obj'));
    assert.equal(bad.status, 1); assert.match(bad.stderr, /Output must be/);
    assert.equal(cli('build', example, '--out', join(temporary, 'part.svg')).status, 1);
    const inspect = cli('inspect', example);
    assert.equal(inspect.status, 0, inspect.stderr);
    assert.equal(JSON.parse(inspect.stdout).bodies.length, 1);
  } finally { await removeTestDirectory(temporary); }
});
