import assert from 'node:assert/strict';
import test from 'node:test';

import {
  chooseWorkspaceDirectory,
  workspaceDirectoryPickerOptions
} from './workspaceDirectoryPicker.js';

test('workspace picker opens one directory without file or multiple selection', async () => {
  let receivedOptions = null;
  const selected = await chooseWorkspaceDirectory(async (options) => {
    receivedOptions = options;
    return '/Users/example/project';
  });

  assert.equal(selected, '/Users/example/project');
  assert.deepEqual(receivedOptions, {
    directory: true,
    multiple: false,
    recursive: false,
    canCreateDirectories: false,
    title: 'Choose project or skills folder'
  });
});

test('workspace picker cancellation returns null without inventing an error', async () => {
  assert.equal(await chooseWorkspaceDirectory(async () => null), null);
});

test('workspace picker rejects invalid non-string selections', async () => {
  await assert.rejects(
    chooseWorkspaceDirectory(async () => ['/Users/example/project']),
    /invalid path/
  );
});

test('workspace picker preserves plugin failures for actionable UI errors', async () => {
  await assert.rejects(
    chooseWorkspaceDirectory(async () => {
      throw new Error('Native dialog unavailable');
    }),
    /Native dialog unavailable/
  );
});
