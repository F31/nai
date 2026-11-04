// Test-only resolution for Windows when an existing workspace link was created
// by WSL and cannot be followed by Node on Windows. No package files are changed.
import {pathToFileURL} from 'node:url';
import path from 'node:path';

export async function resolve(specifier, context, nextResolve) {
  if (specifier === '@nai/contracts') {
    return {url:pathToFileURL(path.resolve('packages/contracts-ts/dist/index.js')).href,shortCircuit:true};
  }
  return nextResolve(specifier, context);
}
