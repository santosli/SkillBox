import assert from 'node:assert/strict';
import test from 'node:test';

import * as workspaceModule from './workspaces.js';

const {
  normalizeWorkspace,
  sidebarItems,
  workspaceCardMetaLabels,
  sidebarFooterItems,
  sidebarIconConvention,
  workspaceMatchesTypeFilter,
  workspaceCounts,
  workspaceDeploymentChanges,
  workspaceDeployPickerRows,
  workspaceDeployRequiresConfirmation,
  workspaceSkillReviewMeta,
  workspaceTypeTabs,
} = workspaceModule;

test('normalizes workspace snake case fields and compact labels', () => {
  const workspace = normalizeWorkspace({
    canonical_path: '/Users/santos/project/.agents/skills',
    path: '/Users/santos/project/.agents/skills',
    kind: 'user',
    source: 'manual',
    agent_id: 'agents',
    display_name: 'Agents User',
    skill_count: 3,
    imported_skill_count: 2,
    usage_count: 7,
    last_scan_error_count: 1,
    last_scan_error: 'one unreadable skill',
    last_scanned_at: '2026-05-26 08:00:00'
  });

  assert.equal(workspace.canonicalPath, '/Users/santos/project/.agents/skills');
  assert.equal(workspace.compactPath, '~/project/.agents/skills');
  assert.equal(workspace.kindLabel, 'User');
  assert.equal(workspace.agentLabel, 'Codex CLI');
  assert.deepEqual(workspace.agentIcon, {
    id: 'workspace:/Users/santos/project/.agents/skills',
    label: 'project',
    iconClass: 'workspace',
    iconLabel: 'P',
    workspace: true
  });
  assert.equal(workspace.skillCount, 3);
  assert.equal(workspace.importedSkillCount, 2);
  assert.equal(workspace.usageCount, 7);
  assert.equal(workspace.lastScanErrorCount, 1);
  assert.equal(workspace.lastScanError, 'one unreadable skill');
});

test('derives workspace display names from agent roots and project directories', () => {
  assert.equal(
    normalizeWorkspace({
      path: '/Users/santos/.codex/skills',
      kind: 'global',
      agent_id: 'codex',
      display_name: 'Codex Global'
    }).displayName,
    'Codex'
  );
  assert.equal(
    normalizeWorkspace({
      path: '/Users/santos/.agents/skills',
      kind: 'global',
      agent_id: 'agents',
      display_name: 'Agents'
    }).displayName,
    'Codex CLI'
  );
  assert.equal(
    normalizeWorkspace({
      path: '/Users/santos/Library/Mobile Documents/iCloud~md~obsidian/Documents/Pandora/.agents/skills',
      kind: 'user',
      agent_id: 'agents',
      display_name: 'Agents User'
    }).displayName,
    'Pandora'
  );
  assert.equal(
    normalizeWorkspace({
      path: '/Users/santos/zone/audio-dialogue-web/.codex/skills',
      kind: 'user',
      agent_id: 'codex'
    }).displayName,
    'audio-dialogue-web'
  );
});

test('derives user workspace icons from display names instead of runtime paths', () => {
  assert.deepEqual(
    normalizeWorkspace({
      path: '/Users/santos/zone/audio-dialogue-web/.codex/skills',
      kind: 'user'
    }).agentIcon,
    {
      id: 'workspace:/Users/santos/zone/audio-dialogue-web/.codex/skills',
      label: 'audio-dialogue-web',
      iconClass: 'workspace',
      iconLabel: 'A',
      workspace: true
    }
  );
});

test('counts workspaces by kind, source, and scan errors', () => {
  const counts = workspaceCounts([
    normalizeWorkspace({ kind: 'global', source: 'auto', skill_count: 2 }),
    normalizeWorkspace({ kind: 'user', source: 'manual', skill_count: 1, imported_skill_count: 1 }),
    normalizeWorkspace({ kind: 'user', source: 'auto', last_scan_error_count: 2 })
  ]);

  assert.deepEqual(counts, {
    total: 3,
    global: 1,
    user: 2,
    manual: 1,
    skills: 3,
    imported: 1,
    errors: 2
  });
});

test('builds workspace type tabs from workspace counts', () => {
  assert.deepEqual(
    workspaceTypeTabs({ total: 4, global: 3, user: 1 }),
    [
      { id: 'all', label: 'All', count: 4 },
      { id: 'global', label: 'Global', count: 3 },
      { id: 'user', label: 'User', count: 1 }
    ]
  );
});

test('filters workspaces by type', () => {
  const globalWorkspace = normalizeWorkspace({ kind: 'global' });
  const userWorkspace = normalizeWorkspace({ kind: 'user' });

  assert.equal(workspaceMatchesTypeFilter(globalWorkspace, 'all'), true);
  assert.equal(workspaceMatchesTypeFilter(globalWorkspace, 'global'), true);
  assert.equal(workspaceMatchesTypeFilter(globalWorkspace, 'user'), false);
  assert.equal(workspaceMatchesTypeFilter(userWorkspace, 'user'), true);
});

test('builds workspace skill review metadata', () => {
  const workspace = normalizeWorkspace({
    path: '/Users/santos/zone/audio-dialogue-web/.codex/skills',
    kind: 'user',
    agent_id: 'codex'
  });

  assert.deepEqual(workspaceSkillReviewMeta(workspace), {
    title: 'audio-dialogue-web skills',
    subtitle: '~/zone/audio-dialogue-web/.codex/skills',
    noticePrefix: 'audio-dialogue-web:'
  });
});

test('builds deploy picker rows with existing deployments checked', () => {
  const rows = workspaceDeployPickerRows(
    [
      normalizeWorkspace({
        canonical_path: '/Users/santos/project/.agents/skills',
        path: '/Users/santos/project/.agents/skills',
        kind: 'user',
        agent_id: 'agents'
      }),
      normalizeWorkspace({
        canonical_path: '/Users/santos/.codex/skills',
        path: '/Users/santos/.codex/skills',
        kind: 'global',
        agent_id: 'codex'
      })
    ],
    [{ target_root: '/Users/santos/project/.agents/skills' }]
  );

  assert.equal(rows[0].isDeployed, true);
  assert.equal(rows[0].isSelected, true);
  assert.equal(rows[1].isDeployed, false);
  assert.equal(rows[1].isSelected, false);
});

test('computes deploy and undeploy changes from picker rows', () => {
  const rows = workspaceDeployPickerRows(
    [
      normalizeWorkspace({
        canonical_path: '/Users/santos/project/.agents/skills',
        path: '/Users/santos/project/.agents/skills'
      }),
      normalizeWorkspace({
        canonical_path: '/Users/santos/.codex/skills',
        path: '/Users/santos/.codex/skills',
        kind: 'global'
      })
    ],
    [{ target_root: '/Users/santos/project/.agents/skills' }]
  );
  rows[0].isSelected = false;
  rows[1].isSelected = true;

  const changes = workspaceDeploymentChanges(rows);

  assert.deepEqual(changes.deploy.map((workspace) => workspace.path), ['/Users/santos/.codex/skills']);
  assert.deepEqual(changes.undeploy.map((workspace) => workspace.path), [
    '/Users/santos/project/.agents/skills'
  ]);
  assert.equal(workspaceDeployRequiresConfirmation(changes), true);
});

test('workspace helpers do not expose a status field formatter', () => {
  assert.equal('workspaceStatusLabel' in workspaceModule, false);
});

test('sidebar keeps primary navigation entries without user or remote entries', () => {
  assert.deepEqual(
    sidebarItems.map((item) => [item.id, item.label, item.icon]),
    [
      ['dashboard', 'Dashboard', 'gauge'],
      ['workspaces', 'Workspaces', 'folder-code'],
      ['history', 'History', 'history']
    ]
  );
});

test('sidebar footer icons follow the lucide-react convention', () => {
  assert.equal(sidebarIconConvention, 'lucide-react');
  assert.deepEqual(
    sidebarFooterItems.map((item) => [item.id, item.label, item.icon]),
    [
      ['settings', 'Settings', 'settings-2'],
      ['help', 'Help', 'message-circle-question-mark']
    ]
  );
});

test('workspace card metadata keeps only user-facing fields', () => {
  assert.deepEqual(workspaceCardMetaLabels, ['Scope', 'Skills', 'Imported', 'Calls']);
  assert.equal('workspaceTableColumns' in workspaceModule, false);
});
