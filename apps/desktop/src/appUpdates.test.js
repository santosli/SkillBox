import assert from 'node:assert/strict';
import test from 'node:test';

import {
  appUpdateNotice,
  normalizeAppUpdateStatus,
  shouldCheckAppUpdateOnStartup
} from './appUpdates.js';

test('normalizes idle app update status from the desktop package version', () => {
  assert.deepEqual(normalizeAppUpdateStatus(null, '0.3.0'), {
    state: 'idle',
    available: false,
    currentVersion: '0.3.0',
    version: '',
    date: '',
    body: '',
    checkedAt: '',
    message: ''
  });
});

test('normalizes available app update metadata from Tauri snake case fields', () => {
  const status = normalizeAppUpdateStatus(
    {
      available: true,
      current_version: '0.2.0',
      version: '0.3.0',
      date: '2026-06-11T10:00:00Z',
      body: '- App auto updates.',
      checked_at: '2026-06-11T10:01:00Z'
    },
    '0.2.0'
  );

  assert.equal(status.state, 'available');
  assert.equal(status.available, true);
  assert.equal(status.currentVersion, '0.2.0');
  assert.equal(status.version, '0.3.0');
  assert.equal(status.body, '- App auto updates.');
  assert.equal(appUpdateNotice(status), 'SkillBox v0.3.0 is available.');
});

test('does not auto check app updates in browser preview or after a completed check', () => {
  assert.equal(
    shouldCheckAppUpdateOnStartup({
      tauriAvailable: false,
      updateStatus: normalizeAppUpdateStatus(null, '0.3.0')
    }),
    false
  );
  assert.equal(
    shouldCheckAppUpdateOnStartup({
      tauriAvailable: true,
      updateStatus: normalizeAppUpdateStatus({ checked_at: '2026-06-11T10:00:00Z' }, '0.3.0')
    }),
    false
  );
  assert.equal(
    shouldCheckAppUpdateOnStartup({
      tauriAvailable: true,
      updateStatus: normalizeAppUpdateStatus(null, '0.3.0')
    }),
    true
  );
});
