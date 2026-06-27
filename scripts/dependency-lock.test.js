import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

function packageLockReactVersions() {
  const lock = JSON.parse(readFileSync(new URL('../package-lock.json', import.meta.url), 'utf8'));
  return [
    ...new Set(
      Object.entries(lock.packages)
        .filter(([path, metadata]) => path.endsWith('node_modules/react') && metadata.version)
        .map(([, metadata]) => metadata.version)
    )
  ].sort();
}

function desktopReactDependencyVersion() {
  const packageJson = JSON.parse(
    readFileSync(new URL('../apps/desktop/package.json', import.meta.url), 'utf8')
  );
  return packageJson.dependencies.react.replace(/^[^\d]*/, '');
}

test('package lock resolves React to a single version', () => {
  assert.deepEqual(packageLockReactVersions(), [desktopReactDependencyVersion()]);
});
