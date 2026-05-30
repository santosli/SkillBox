import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

test('git hook installer fails when hooks path cannot be configured', () => {
  const source = readFileSync('scripts/install-git-hooks.js', 'utf8');

  assert.match(source, /process\.exitCode\s*=\s*1/);
  assert.doesNotMatch(source, /process\.exitCode\s*=\s*strictInstall\s*\?\s*1\s*:\s*0/);
});
