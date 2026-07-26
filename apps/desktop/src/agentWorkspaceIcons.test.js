import assert from 'node:assert/strict';
import test from 'node:test';

import {
  agentWorkspaceIconForId,
  agentWorkspaceLabel,
  workspaceAgentIcon
} from './agentWorkspaceIcons.js';

test('maps Rust runtime profile ids to stable workspace icons', () => {
  assert.deepEqual(agentWorkspaceIconForId('agents'), {
    id: 'agents',
    label: 'Agents',
    iconClass: 'codex-cli',
    iconAsset: 'codex-cli'
  });
  assert.equal(agentWorkspaceLabel('agents'), 'Agents');
  assert.deepEqual(agentWorkspaceIconForId('codex'), {
    id: 'codex',
    label: 'Codex',
    iconClass: 'codex-app',
    iconAsset: 'codex-app'
  });
  assert.deepEqual(agentWorkspaceIconForId('claude-code'), {
    id: 'claude-code',
    label: 'Claude Code',
    iconClass: 'claude-code',
    iconAsset: 'claude-code'
  });
  assert.deepEqual(agentWorkspaceIconForId('custom-skill-md'), {
    id: 'custom-skill-md',
    label: 'Custom SKILL.md'
  });
});

test('workspace icons consume profile metadata instead of path markers', () => {
  assert.deepEqual(
    workspaceAgentIcon({
      profile_id: 'cursor',
      path: '/arbitrary/path/without/runtime/markers'
    }),
    { id: 'cursor', label: 'Cursor' }
  );
  assert.equal(agentWorkspaceIconForId('/Users/example/.codex/skills'), null);
});
