import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const css = await readFile(new URL('./styles.css', import.meta.url), 'utf8');
const colorsCss = await readFile(new URL('./colors.css', import.meta.url), 'utf8');
const appSourcePaths = [
  './App.jsx',
  './components/dashboard.jsx',
  './components/common.jsx',
  './components/workspaces.jsx',
  './components/history.jsx',
  './components/rankings.jsx',
  './components/settings.jsx',
  './components/importReview.jsx',
  './components/skillDetail.jsx',
  './components/remoteSkills.jsx',
  './components/userSkillsSync.jsx',
  './skills.js',
  './historyEntries.js',
  './usageRankings.js',
  './usageHooks.js',
  './workspaces.js',
  './appUpdates.js',
  './preferences.js',
  './previewData.js',
  './importFlow.js',
  './workspaceDirectoryPicker.js'
];
const appSource = (
  await Promise.all(
    appSourcePaths.map((path) => readFile(new URL(path, import.meta.url), 'utf8'))
  )
).join('\n');
const appComponentSource = await readFile(new URL('./App.jsx', import.meta.url), 'utf8');
const mainSource = await readFile(new URL('./main.jsx', import.meta.url), 'utf8');
const tauriSource = await readFile(new URL('../src-tauri/src/lib.rs', import.meta.url), 'utf8');
const tauriMainSource = await readFile(new URL('../src-tauri/src/main.rs', import.meta.url), 'utf8');
const tauriCargo = await readFile(new URL('../src-tauri/Cargo.toml', import.meta.url), 'utf8');
const tauriMainCapability = JSON.parse(
  await readFile(new URL('../src-tauri/capabilities/main.json', import.meta.url), 'utf8')
);
const desktopPackage = JSON.parse(
  await readFile(new URL('../package.json', import.meta.url), 'utf8')
);

test('all current and future text fields disable automatic writing assistance', () => {
  assert.match(mainSource, /node\.matches\('input, textarea'\)/);
  assert.match(mainSource, /setAttribute\('autocapitalize', 'none'\)/);
  assert.match(mainSource, /setAttribute\('autocomplete', 'off'\)/);
  assert.match(mainSource, /setAttribute\('autocorrect', 'off'\)/);
  assert.match(mainSource, /setAttribute\('spellcheck', 'false'\)/);
  assert.match(mainSource, /new MutationObserver/);
  assert.match(mainSource, /observe\(rootElement, \{ childList: true, subtree: true \}\)/);
});
