import { type CadDocument, parseDocument } from './document.js';
import { BuiltModel, buildModel } from './model.js';

/** Atomic edits: recompute succeeds before document, geometry, or history changes. */
export class ModelSession {
  #undo: CadDocument[] = [];
  #redo: CadDocument[] = [];
  #busy = false;
  #disposed = false;

  private constructor(private currentDocument: CadDocument, private currentModel: BuiltModel) {}

  static async create(input: unknown): Promise<ModelSession> {
    const document = parseDocument(input);
    return new ModelSession(document, await buildModel(document));
  }

  get document(): CadDocument { return structuredClone(this.currentDocument); }
  get model(): BuiltModel { return this.currentModel; }
  get canUndo(): boolean { return this.#undo.length > 0; }
  get canRedo(): boolean { return this.#redo.length > 0; }

  private async commit(candidate: unknown, action: 'edit' | 'undo' | 'redo'): Promise<void> {
    if (this.#disposed) throw new Error('Session has been disposed');
    if (this.#busy) throw new Error('An edit is already in progress; await it before editing again');
    this.#busy = true;
    try {
      const document = parseDocument(candidate);
      const model = await buildModel(document);
      if (action === 'edit') {
        this.#undo.push(this.currentDocument);
        if (this.#undo.length > 100) this.#undo.shift();
        this.#redo = [];
      } else if (action === 'undo') {
        this.#undo.pop(); this.#redo.push(this.currentDocument);
      } else {
        this.#redo.pop(); this.#undo.push(this.currentDocument);
      }
      this.currentModel.dispose();
      this.currentDocument = document;
      this.currentModel = model;
    } finally { this.#busy = false; }
  }

  async replace(input: unknown): Promise<void> { await this.commit(input, 'edit'); }

  async setParameter(name: string, value: number): Promise<void> {
    const candidate = this.document;
    if (!Object.hasOwn(candidate.parameters, name)) throw new Error(`Unknown parameter "${name}"`);
    candidate.parameters[name] = value;
    await this.commit(candidate, 'edit');
  }

  async suppressFeature(id: string, suppressed: boolean): Promise<void> {
    const candidate = this.document;
    const feature = candidate.features.find(f => f.id === id);
    if (!feature) throw new Error(`Unknown feature "${id}"`);
    feature.suppressed = suppressed;
    await this.commit(candidate, 'edit');
  }

  async undo(): Promise<boolean> {
    const document = this.#undo.at(-1);
    if (!document) return false;
    await this.commit(document, 'undo'); return true;
  }

  async redo(): Promise<boolean> {
    const document = this.#redo.at(-1);
    if (!document) return false;
    await this.commit(document, 'redo'); return true;
  }

  dispose(): void {
    if (this.#busy) throw new Error('Cannot dispose while an edit is in progress');
    this.currentModel.dispose(); this.#disposed = true;
  }
}
