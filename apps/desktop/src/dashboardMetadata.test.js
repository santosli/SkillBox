import assert from 'node:assert/strict';
import test from 'node:test';

import {
  dashboardFilterOptions,
  deriveDashboardSkill,
  normalizeDashboardTagOverrides,
  normalizeFavoriteNames
} from './dashboardMetadata.js';
import { normalizeRemoteSkillUpdates } from './skillStatusRefresh.js';

test('derives dashboard labels, status, and favorite state without generated tags', () => {
  const remoteUpdates = normalizeRemoteSkillUpdates({
    statuses: [
      { skill_name: 'note-manager', state: 'update_available', update_available: true }
    ]
  });
  const skill = deriveDashboardSkill(
    {
      name: 'note-manager',
      description: 'Manage Obsidian docs and sync notes from GitHub.',
      sourceRoot: '/Users/example/.codex/skills',
      type: 'remote',
      status: 'update not checked'
    },
    { state: 'clean' },
    remoteUpdates,
    new Set(['note-manager'])
  );

  assert.equal(skill.agentLabel, 'Codex');
  assert.equal(skill.sourceLabel, '~/.codex/skills');
  assert.deepEqual(skill.installedAgents, []);
  assert.equal(skill.statusLabel, 'Update available');
  assert.equal(skill.statusTone, 'amber');
  assert.equal(skill.isFavorite, true);
  assert.deepEqual(skill.displayTags, []);
});

test('derives user skill sync status without a default tag', () => {
  const skill = deriveDashboardSkill(
    {
      name: 'alpha',
      description: 'Small helper.',
      sourceRoot: '/Users/example/.agents/skills',
      type: 'user'
    },
    { state: 'dirty', changedPaths: ['beta/SKILL.md'] },
    normalizeRemoteSkillUpdates(null),
    new Set()
  );

  assert.equal(skill.agentLabel, 'Codex CLI');
  assert.deepEqual(skill.installedAgents, []);
  assert.equal(skill.statusLabel, 'Synced');
  assert.equal(skill.statusTone, 'green');
  assert.equal(skill.isFavorite, false);
  assert.deepEqual(skill.displayTags, []);
});

test('does not derive tags from skill name and description', () => {
  const skill = deriveDashboardSkill(
    {
      name: 'alpha',
      description: 'Small helper.',
      sourceRoot: '/Users/santos/github/sync-tools/.codex/skills',
      source_root: '/Users/santos/github/sync-tools/.codex/skills',
      type: 'remote',
      status: 'update available'
    },
    { state: 'clean' },
    normalizeRemoteSkillUpdates(null),
    new Set()
  );

  assert.deepEqual(skill.displayTags, []);
});

test('managed current symlinks do not count as active workspace deployments', () => {
  const skill = deriveDashboardSkill(
    {
      name: 'last30days',
      description: 'Research what people actually say about any topic in the last 30 days.',
      sourceRoot: '/Users/example/.skillbox/remote-skills',
      type: 'remote',
      isSymlink: true,
      deployments: []
    },
    { state: 'clean' },
    normalizeRemoteSkillUpdates(null),
    new Set()
  );

  assert.deepEqual(skill.installedAgents, []);
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
        { target_root: '/Users/example/.cursor/skills' },
        { agentId: 'claude' }
      ]
    },
    { state: 'clean' },
    normalizeRemoteSkillUpdates(null),
    new Set()
  );

  assert.deepEqual(skill.installedAgents, [
    { id: 'claude', label: 'Claude Code', iconClass: 'claude-code', iconAsset: 'claude-code' },
    { id: 'codex', label: 'Codex', iconClass: 'codex-app', iconAsset: 'codex-app' },
    { id: 'cursor', label: 'Cursor' }
  ]);
});

test('keeps separate workspace deployment icons for the same agent runtime', () => {
  const skill = deriveDashboardSkill(
    {
      name: 'ui-ux-pro-max',
      description: 'UI/UX design intelligence.',
      type: 'remote',
      deployments: [
        { target_root: '/Users/example/.codex/skills' },
        { target_root: '/Users/example/zone/demo-app/.codex/skills' }
      ]
    },
    { state: 'clean' },
    normalizeRemoteSkillUpdates(null),
    new Set(),
    {},
    [
      {
        path: '/Users/example/.codex/skills',
        canonical_path: '/Users/example/.codex/skills',
        kind: 'global',
        agent_id: 'codex',
        display_name: 'Codex'
      },
      {
        path: '/Users/example/zone/demo-app/.codex/skills',
        canonical_path: '/Users/example/zone/demo-app/.codex/skills',
        kind: 'user',
        agent_id: 'codex',
        display_name: 'demo-app'
      }
    ]
  );

  assert.deepEqual(skill.installedAgents, [
    { id: 'codex', label: 'Codex', iconClass: 'codex-app', iconAsset: 'codex-app' },
    {
      id: 'workspace:/Users/example/zone/demo-app/.codex/skills',
      label: 'demo-app',
      iconClass: 'workspace',
      iconLabel: 'D',
      workspace: true
    }
  ]);
});

test('uses the Codex CLI icon for global agents runtime deployments', () => {
  const skill = deriveDashboardSkill(
    {
      name: 'git-merge-to-main',
      description: 'Merge branches after review.',
      type: 'user',
      deployments: [{ target_root: '/Users/example/.agents/skills' }]
    },
    { state: 'clean' },
    normalizeRemoteSkillUpdates(null),
    new Set(),
    {},
    [
      {
        path: '/Users/example/.agents/skills',
        canonical_path: '/Users/example/.agents/skills',
        kind: 'global',
        agent_id: 'agents',
        display_name: 'Agents'
      }
    ]
  );

  assert.deepEqual(skill.installedAgents, [
    { id: 'agents', label: 'Codex CLI', iconClass: 'codex-cli', iconAsset: 'codex-cli' }
  ]);
});

test('uses editable dashboard tag overrides when present', () => {
  const skill = deriveDashboardSkill(
    {
      name: 'note-manager',
      description: 'Manage Obsidian docs and sync notes from GitHub.',
      sourceRoot: '/Users/example/.codex/skills',
      type: 'remote'
    },
    { state: 'clean' },
    normalizeRemoteSkillUpdates(null),
    new Set(),
    { 'note-manager': ['writing', 'sync'] }
  );

  assert.deepEqual(skill.displayTags, ['writing', 'sync']);
});

test('normalizes editable dashboard tag overrides from persisted values', () => {
  assert.deepEqual(
    normalizeDashboardTagOverrides(
      JSON.stringify({
        alpha: [' Sync ', 'SYNC', 'research notes', 3],
        beta: [],
        '': ['ignored'],
        gamma: 'not-tags'
      })
    ),
    {
      alpha: ['sync', 'research-notes'],
      beta: []
    }
  );
  assert.deepEqual(normalizeDashboardTagOverrides('not-json'), {});
});

test('does not infer installed agents from managed current symlink status', () => {
  const skill = deriveDashboardSkill(
    {
      name: 'managed-skill',
      description: 'Managed SkillBox copy.',
      sourceRoot: '/Users/example/.skillbox/user-skills',
      type: 'user',
      isSymlink: true
    },
    { state: 'clean' },
    normalizeRemoteSkillUpdates(null),
    new Set()
  );

  assert.equal(skill.agentLabel, 'Local');
  assert.deepEqual(skill.installedAgents, []);
});

test('builds stable dashboard filter options from derived skills', () => {
  const skills = [
    { displayTags: ['sync', 'github'], agentLabel: 'Codex' },
    { displayTags: [], agentLabel: 'Codex CLI' },
    { displayTags: ['sync'], agentLabel: 'Local' }
  ];

  assert.deepEqual(dashboardFilterOptions(skills), {
    tags: ['sync', 'github'],
    agents: ['Codex', 'Codex CLI', 'Local']
  });
});

test('normalizes favorite names from persisted values', () => {
  assert.deepEqual(normalizeFavoriteNames('["alpha","beta",3,null,"alpha"]'), ['alpha', 'beta']);
  assert.deepEqual(normalizeFavoriteNames('not-json'), []);
  assert.deepEqual(normalizeFavoriteNames(['gamma', '', 'delta']), ['gamma', 'delta']);
});
