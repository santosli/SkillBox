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

test('desktop Tauri restarts return to the Dock without stealing focus', () => {
  const desktopPackage = JSON.parse(readFileSync('apps/desktop/package.json', 'utf8'));
  const devConfig = JSON.parse(
    readFileSync('apps/desktop/src-tauri/tauri.dev.conf.json', 'utf8')
  );
  const rustSource = readFileSync('apps/desktop/src-tauri/src/lib.rs', 'utf8');

  assert.equal(
    desktopPackage.scripts['tauri:dev'],
    'tauri dev --config src-tauri/tauri.dev.conf.json'
  );
  assert.deepEqual(devConfig.app.windows, []);
  assert.equal(devConfig.productName, 'SkillBox Dev');
  assert.equal(devConfig.identifier, 'io.github.santosli.skillbox.dev');
  assert.match(rustSource, /#\[cfg\(debug_assertions\)\]\s+fn create_development_window/);
  assert.match(
    rustSource,
    /app\.set_activation_policy\(tauri::ActivationPolicy::Accessory\);/
  );
  assert.match(
    rustSource,
    /app\.handle\(\)\s*\.set_activation_policy\(tauri::ActivationPolicy::Regular\)\?;/
  );
  assert.match(rustSource, /fn development_frontmost_application_pid\(\) -> Option<i32>/);
  assert.match(rustSource, /fn restore_frontmost_application\(pid: Option<i32>\)/);
  assert.match(
    rustSource,
    /restore_frontmost_application\(previous_frontmost_application_pid\);/
  );
  assert.match(rustSource, /\.focused\(false\)/);
  assert.match(rustSource, /\.title\("SkillBox Dev"\)/);
  assert.match(
    rustSource,
    /#\[cfg\(debug_assertions\)\]\s+create_development_window\(app, previous_frontmost_application_pid\)\?;/
  );
});
