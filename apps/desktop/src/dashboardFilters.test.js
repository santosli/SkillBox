import assert from 'node:assert/strict';
import test from 'node:test';

import { dashboardTabItems, skillMatchesDashboardFilter } from './dashboardFilters.js';
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
