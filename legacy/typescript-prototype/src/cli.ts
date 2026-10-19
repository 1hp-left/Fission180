import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, extname, resolve } from 'node:path';
import { parseArgs } from 'node:util';
import { z } from 'zod';
import { parseDocument } from './document.js';
import { buildModel } from './model.js';

const help = `OpenFusion — parametric solid modeling (millimetres)

  npm run cad -- inspect <model.json> [--set name=value]
  npm run cad -- build <model.json> --out <part.step|part.stl>
  npm run cad -- build <model.json> --out <sketch.svg> --sketch <id>

Options:
  -o, --out FILE       Write STEP, binary STL, or a sketch SVG
  --set NAME=VALUE     Override an existing parameter; may be repeated
  --save FILE          Save the updated parametric JSON document
  --sketch ID          Select an active sketch for SVG output
  --force              Allow overwriting output files
  -h, --help           Show this help
`;

async function main(): Promise<void> {
  const { values, positionals } = parseArgs({
    allowPositionals: true,
    options: {
      out: { type: 'string', short: 'o' }, set: { type: 'string', multiple: true },
      save: { type: 'string' }, sketch: { type: 'string' },
      force: { type: 'boolean', default: false }, help: { type: 'boolean', short: 'h' },
    },
  });
  if (values.help || positionals.length === 0) { console.log(help); return; }
  const [command, input] = positionals;
  if (!['inspect', 'build'].includes(command!) || !input || positionals.length !== 2) throw new Error('Expected inspect or build followed by one model file. Use --help.');
  if (command === 'build' && !values.out) throw new Error('build requires --out');
  if (command === 'inspect' && (values.out || values.sketch)) throw new Error('Use build for geometry exports');
  const extension = values.out ? extname(values.out).toLowerCase() : '';
  if (values.out && !['.step', '.stp', '.stl', '.svg'].includes(extension)) throw new Error('Output must be .step, .stp, .stl, or .svg');
  if (extension === '.svg' && !values.sketch) throw new Error('SVG export requires --sketch <id>');
  if (values.sketch && extension !== '.svg') throw new Error('--sketch is only used with SVG output');
  if (values.save && values.out && resolve(values.save) === resolve(values.out)) throw new Error('--save and --out must use different paths');
  const bytes = await readFile(input);
  if (bytes.length > 10 * 1024 * 1024) throw new Error('Model JSON exceeds 10 MiB');
  const document = parseDocument(JSON.parse(bytes.toString('utf8')));
  for (const assignment of values.set ?? []) {
    const match = /^([A-Za-z][A-Za-z0-9_-]{0,63})=(.+)$/.exec(assignment);
    if (!match) throw new Error(`Invalid parameter override "${assignment}"; use name=value`);
    const [, name, rawValue] = match;
    if (!Object.hasOwn(document.parameters, name!)) throw new Error(`Unknown parameter "${name}"`);
    const value = Number(rawValue);
    if (!Number.isFinite(value)) throw new Error(`Parameter "${name}" must be a finite number`);
    document.parameters[name!] = value;
  }
  const updated = parseDocument(document);
  const model = await buildModel(updated);
  try {
    const outputs: { path: string; data: string | Uint8Array }[] = [];
    if (values.out) {
      const data = extension === '.svg' ? model.sketchSVG(values.sketch!)
        : new Uint8Array(await (extension === '.stl' ? model.exportSTL() : model.exportSTEP()).arrayBuffer());
      outputs.push({ path: values.out, data });
    }
    if (values.save) outputs.push({ path: values.save, data: JSON.stringify(updated, null, 2) + '\n' });
    // A model input is never implicitly overwritten by geometry export.
    for (const output of outputs) {
      if (resolve(output.path) === resolve(input) && output.path === values.out) throw new Error('Geometry output must differ from the model input');
    }
    for (const output of outputs) {
      await mkdir(dirname(resolve(output.path)), { recursive: true });
      await writeFile(output.path, output.data, { flag: values.force ? 'w' : 'wx' });
    }
    console.log(JSON.stringify({ ...model.summary(), written: outputs.map(o => resolve(o.path)) }, null, 2));
  } finally { model.dispose(); }
}

main().catch((error: unknown) => {
  const message = error instanceof z.ZodError ? z.prettifyError(error) : error instanceof Error ? error.message : String(error);
  console.error(`OpenFusion: ${message}`);
  process.exitCode = 1;
});
