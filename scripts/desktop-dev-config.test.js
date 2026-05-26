import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

test('desktop dev server stays pinned to the Tauri devUrl port', () => {
  const desktopPackage = JSON.parse(readFileSync('apps/desktop/package.json', 'utf8'));
  const tauriConfig = JSON.parse(readFileSync('apps/desktop/src-tauri/tauri.conf.json', 'utf8'));
  const devScript = desktopPackage.scripts.dev;
  const devUrl = new URL(tauriConfig.build.devUrl);

  assert.match(devScript, new RegExp(`--port\\s+${devUrl.port}`));
  assert.match(devScript, /--strictPort\b/);
});
