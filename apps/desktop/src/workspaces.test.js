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
    last_scan_error_count: 1,
    last_scan_error: 'one unreadable skill',
    last_scanned_at: '2026-05-26 08:00:00'
  });

  assert.equal(workspace.canonicalPath, '/Users/santos/project/.agents/skills');
  assert.equal(workspace.compactPath, '~/project/.agents/skills');
  assert.equal(workspace.kindLabel, 'User');
  assert.equal(workspace.agentLabel, 'Agents');
  assert.equal(workspace.skillCount, 3);
  assert.equal(workspace.importedSkillCount, 2);
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

test('workspace helpers do not expose a status field formatter', () => {
  assert.equal('workspaceStatusLabel' in workspaceModule, false);
});

test('sidebar keeps dashboard and workspaces without user or remote entries', () => {
  assert.deepEqual(
    sidebarItems.map((item) => [item.id, item.label, item.icon]),
    [
      ['dashboard', 'Dashboard', 'gauge'],
      ['workspaces', 'Workspaces', 'folder-code']
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
  assert.deepEqual(workspaceCardMetaLabels, ['Scope', 'Skills', 'Imported']);
  assert.equal('workspaceTableColumns' in workspaceModule, false);
});
