import initOpenCascade, { type OpenCascadeInstance } from 'replicad-opencascadejs';
import { setOC } from 'replicad';

let started: Promise<OpenCascadeInstance> | undefined;

/** One kernel per process. No target application or external service is loaded. */
export function initializeKernel(): Promise<OpenCascadeInstance> {
  started ??= initOpenCascade({ print: () => {}, printErr: () => {} }).then((oc) => {
    setOC(oc);
    return oc;
  });
  return started;
}
