import assert from 'node:assert/strict';
import test from 'node:test';

import {
  dashboardTabItems,
  skillMatchesDashboardFilter,
  skillMatchesDashboardFilters,
  sortDashboardSkills
} from './dashboardFilters.js';
import { normalizeRemoteSkillUpdates } from './skillStatusRefresh.js';

test('dashboard tabs expose counts for all skill categories', () => {
  const tabs = dashboardTabItems({ total: 6, user: 2, remote: 4, updates: 1 });

  assert.deepEqual(
    tabs.map((tab) => [tab.id, tab.label, tab.count]),
    [
      ['all', 'All', 6],
      ['user', 'User', 2],
      ['remote', 'Remote', 4],
      ['updates', 'Updates', 1]
    ]
  );
  assert.equal(tabs.some((tab) => 'hint' in tab), false);
});

test('updates filter matches only remote skills with an available update', () => {
  const updates = normalizeRemoteSkillUpdates({
    statuses: [
      { skill_name: 'find-skills', state: 'update_available', update_available: true },
      { skill_name: 'frontend-design', state: 'up_to_date', update_available: false }
    ]
  });

  assert.equal(skillMatchesDashboardFilter({ name: 'local', type: 'user' }, 'updates', updates), false);
  assert.equal(skillMatchesDashboardFilter({ name: 'find-skills', type: 'remote' }, 'updates', updates), true);
  assert.equal(skillMatchesDashboardFilter({ name: 'frontend-design', type: 'remote' }, 'updates', updates), false);
});

test('dashboard combined filters match query, type, tag, agent, and favorites', () => {
  const skills = [
    {
      name: 'note-manager',
      description: 'Manage Obsidian notes',
      type: 'remote',
      displayTags: ['manage', 'obsidian'],
      agentLabel: 'Codex',
      isFavorite: true
    },
    {
      name: 'alpha',
      description: 'Small helper',
      type: 'user',
      displayTags: ['general'],
      agentLabel: 'Agents',
      isFavorite: false
    },
    {
      name: 'github-sync',
      description: 'Sync GitHub skills',
      type: 'remote',
      displayTags: ['github', 'sync'],
      agentLabel: 'Local',
      isFavorite: false
    }
  ];

  assert.deepEqual(
    skills
      .filter((skill) =>
        skillMatchesDashboardFilters(skill, {
          type: 'remote',
          query: 'obsidian',
          tag: 'manage',
          agent: 'Codex',
          favoritesOnly: true
        })
      )
      .map((skill) => skill.name),
    ['note-manager']
  );

  assert.deepEqual(
    skills
      .filter((skill) =>
        skillMatchesDashboardFilters(skill, {
          type: 'all',
          query: 'sync',
          tag: 'all',
          agent: 'all',
          favoritesOnly: false
        })
      )
      .map((skill) => skill.name),
    ['github-sync']
  );
});

test('dashboard skills sort favorites first then by name', () => {
  const skills = [
    { name: 'zeta-remote', type: 'remote', isFavorite: true },
    { name: 'beta-user', type: 'user' },
    { name: 'alpha-remote', type: 'remote' },
    { name: 'Alpha-user', type: 'user', isFavorite: true }
  ];

  const sorted = sortDashboardSkills(skills);

  assert.deepEqual(
    sorted.map((skill) => skill.name),
    ['Alpha-user', 'zeta-remote', 'alpha-remote', 'beta-user']
  );
  assert.deepEqual(
    skills.map((skill) => skill.name),
    ['zeta-remote', 'beta-user', 'alpha-remote', 'Alpha-user']
  );
});
