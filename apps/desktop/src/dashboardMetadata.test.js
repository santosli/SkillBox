import assert from 'node:assert/strict';
import test from 'node:test';

import {
  dashboardFilterOptions,
  deriveDashboardSkill,
  normalizeFavoriteNames
} from './dashboardMetadata.js';
import { normalizeRemoteSkillUpdates } from './skillStatusRefresh.js';

test('derives dashboard tags, agent label, source label, status, and favorite state', () => {
  const remoteUpdates = normalizeRemoteSkillUpdates({
    statuses: [
      { skill_name: 'note-manager', state: 'update_available', update_available: true }
    ]
  });
  const skill = deriveDashboardSkill(
    {
      name: 'note-manager',
      description: 'Manage Obsidian docs and sync notes from GitHub.',
      sourceRoot: '/Users/santos/.codex/skills',
      type: 'remote',
      status: 'update not checked'
    },
    { state: 'clean' },
    remoteUpdates,
    new Set(['note-manager'])
  );

  assert.equal(skill.agentLabel, 'Codex');
  assert.equal(skill.sourceLabel, '~/.codex/skills');
  assert.deepEqual(skill.installedAgents, [{ id: 'codex', label: 'Codex' }]);
  assert.equal(skill.statusLabel, 'Update available');
  assert.equal(skill.statusTone, 'amber');
  assert.equal(skill.isFavorite, true);
  assert.deepEqual(skill.displayTags, ['manage', 'doc', 'obsidian', 'github', 'sync']);
});

test('derives user skill sync status and default general tag', () => {
  const skill = deriveDashboardSkill(
    {
      name: 'alpha',
      description: 'Small helper.',
      sourceRoot: '/Users/santos/.agents/skills',
      type: 'user'
    },
    { state: 'dirty', changedPaths: ['beta/SKILL.md'] },
    normalizeRemoteSkillUpdates(null),
    new Set()
  );

  assert.equal(skill.agentLabel, 'Agents');
  assert.deepEqual(skill.installedAgents, [{ id: 'agents', label: 'Agents' }]);
  assert.equal(skill.statusLabel, 'Synced');
  assert.equal(skill.statusTone, 'green');
  assert.equal(skill.isFavorite, false);
  assert.deepEqual(skill.displayTags, ['general']);
});

test('derives installed agent icons from explicit agent and deployment fields', () => {
  const skill = deriveDashboardSkill(
    {
      name: 'multi-agent',
      description: 'Deploy into multiple agent runtimes.',
      type: 'remote',
      installedAgents: ['claude'],
      deployments: [
        { agent_id: 'codex' },
        { target_root: '/Users/santos/.cursor/skills' },
        { agentId: 'claude' }
      ]
    },
    { state: 'clean' },
    normalizeRemoteSkillUpdates(null),
    new Set()
  );

  assert.deepEqual(skill.installedAgents, [
    { id: 'claude', label: 'Claude' },
    { id: 'codex', label: 'Codex' },
    { id: 'cursor', label: 'Cursor' }
  ]);
});

test('falls back to the current symlink deployment target for installed agent icons', () => {
  const skill = deriveDashboardSkill(
    {
      name: 'managed-skill',
      description: 'Managed SkillBox copy.',
      sourceRoot: '/Users/santos/SkillBox/user-skills',
      type: 'user',
      isSymlink: true
    },
    { state: 'clean' },
    normalizeRemoteSkillUpdates(null),
    new Set()
  );

  assert.equal(skill.agentLabel, 'Local');
  assert.deepEqual(skill.installedAgents, [{ id: 'codex', label: 'Codex' }]);
});

test('builds stable dashboard filter options from derived skills', () => {
  const skills = [
    { displayTags: ['sync', 'github'], agentLabel: 'Codex' },
    { displayTags: ['general'], agentLabel: 'Agents' },
    { displayTags: ['sync'], agentLabel: 'Local' }
  ];

  assert.deepEqual(dashboardFilterOptions(skills), {
    tags: ['sync', 'github', 'general'],
    agents: ['Codex', 'Agents', 'Local']
  });
});

test('normalizes favorite names from persisted values', () => {
  assert.deepEqual(normalizeFavoriteNames('["alpha","beta",3,null,"alpha"]'), ['alpha', 'beta']);
  assert.deepEqual(normalizeFavoriteNames('not-json'), []);
  assert.deepEqual(normalizeFavoriteNames(['gamma', '', 'delta']), ['gamma', 'delta']);
});
