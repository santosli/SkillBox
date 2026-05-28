import assert from 'node:assert/strict';
import test from 'node:test';

import {
  agentWorkspaceIconForId,
  agentWorkspaceIconForPath,
  agentWorkspaceLabel
} from './agentWorkspaceIcons.js';

test('maps the legacy agents runtime to the Codex CLI icon', () => {
  assert.deepEqual(agentWorkspaceIconForId('agents'), {
    id: 'agents',
    label: 'Codex CLI',
    iconClass: 'codex-cli',
    iconAsset: 'codex-cli'
  });
  assert.equal(agentWorkspaceLabel('agents'), 'Codex CLI');
});

test('maps the Codex app runtime to the mac app icon', () => {
  assert.deepEqual(agentWorkspaceIconForId('codex'), {
    id: 'codex',
    label: 'Codex',
    iconClass: 'codex-app',
    iconAsset: 'codex-app'
  });
});

test('resolves common global workspace icons from runtime paths', () => {
  assert.deepEqual(agentWorkspaceIconForPath('/Users/santos/.agents/skills'), {
    id: 'agents',
    label: 'Codex CLI',
    iconClass: 'codex-cli',
    iconAsset: 'codex-cli'
  });
  assert.deepEqual(agentWorkspaceIconForPath('/Users/santos/.codex/skills'), {
    id: 'codex',
    label: 'Codex',
    iconClass: 'codex-app',
    iconAsset: 'codex-app'
  });
});
