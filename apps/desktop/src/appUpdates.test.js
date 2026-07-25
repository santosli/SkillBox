import assert from 'node:assert/strict';
import test from 'node:test';

import {
  appUpdateNotice,
  appUpdateStatusAfterCheckError,
  normalizeAppUpdateStatus,
  previewAppUpdateStatus,
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

test('keeps an existing update reminder when a later background check fails', () => {
  const available = normalizeAppUpdateStatus(
    {
      available: true,
      current_version: '0.4.5',
      version: '0.5.0',
      checked_at: '1784954400',
      message: 'App update available.'
    },
    '0.4.5'
  );

  assert.deepEqual(
    appUpdateStatusAfterCheckError(
      available,
      'Network unavailable.',
      '0.4.5',
      '2026-07-25T12:00:00Z'
    ),
    available
  );

  const failed = appUpdateStatusAfterCheckError(
    normalizeAppUpdateStatus(null, '0.4.5'),
    'Network unavailable.',
    '0.4.5',
    '2026-07-25T12:00:00Z'
  );
  assert.equal(failed.state, 'error');
  assert.equal(failed.message, 'Network unavailable.');
});

test('creates an update reminder only from a valid development preview query', () => {
  const preview = previewAppUpdateStatus('?previewAppUpdate=0.5.0', '0.4.5');

  assert.equal(preview?.state, 'available');
  assert.equal(preview?.currentVersion, '0.4.5');
  assert.equal(preview?.version, '0.5.0');
  assert.equal(previewAppUpdateStatus('?previewAppUpdate=latest', '0.4.5'), null);
  assert.equal(previewAppUpdateStatus('', '0.4.5'), null);
});
