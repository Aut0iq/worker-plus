// Copies generated API DTO types from shared/api-types into src/api/types.
// Rewrites the relative JsonValue import (serde_json types are exported next to
// the crate, outside shared/api-types) to a local declaration.
import {
  mkdirSync,
  rmSync,
  readdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const src = join(root, 'shared', 'api-types');
const dst = join(root, 'admin', 'src', 'api', 'types');

rmSync(dst, { recursive: true, force: true });
mkdirSync(dst, { recursive: true });
let n = 0;
for (const f of readdirSync(src)) {
  if (f.endsWith('.ts') && statSync(join(src, f)).isFile()) {
    let text = readFileSync(join(src, f), 'utf8');
    text = text.replaceAll('../../server/serde_json/JsonValue', './JsonValue');
    writeFileSync(join(dst, f), text);
    n++;
  }
}
writeFileSync(
  join(dst, 'JsonValue.ts'),
  'export type JsonValue = string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue };\n',
);
console.log(`synced ${n} types -> ${dst}`);
