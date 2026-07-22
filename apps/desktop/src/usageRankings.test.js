import assert from 'node:assert/strict';
import test from 'node:test';

import {
  defaultUsageRankingFilters,
  normalizeUsageRankings,
  usageRankingAgentOptions,
  usageRankingRequest,
  usageRankingWorkspaceOptions
} from './usageRankings.js';

test('normalizes ranking rows and snake case result metadata', () => {
  assert.deepEqual(
    normalizeUsageRankings({
      generated_at: '2026-07-22T10:00:00Z',
      range: 'last_30_days',
      range_start: '2026-06-22T10:00:00Z',
      range_end: '2026-07-22T10:00:00Z',
      total_observed_calls: 12,
      rows: [
        {
          rank: 1,
          skill_name: 'grill-me',
          kind: 'remote',
          managed: true,
          usage_count: 12,
          last_used_at: '2026-07-21T08:00:00Z'
        }
      ]
    }),
    {
      generatedAt: '2026-07-22T10:00:00Z',
      range: 'last_30_days',
      rangeStart: '2026-06-22T10:00:00Z',
      rangeEnd: '2026-07-22T10:00:00Z',
      agentId: '',
      workspaceRoot: '',
      totalObservedCalls: 12,
      rows: [
        {
          rank: 1,
          skillName: 'grill-me',
          kind: 'remote',
          managed: true,
          usageCount: 12,
          lastUsedAt: '2026-07-21T08:00:00Z'
        }
      ]
    }
  );
});

test('builds local managed-only ranking requests', () => {
  assert.deepEqual(usageRankingRequest(defaultUsageRankingFilters), {
    range: 'last_30_days',
    agentId: null,
    workspaceRoot: null,
    includeUnmanaged: false
  });
});

test('builds unique agent and workspace filter options', () => {
  const hooks = [
    { sharedConfigKey: 'codex', label: 'Codex App' },
    { sharedConfigKey: 'codex', label: 'Codex CLI' },
    { sharedConfigKey: 'claude-code', label: 'Claude Code CLI' }
  ];
  const workspaces = [
    {
      agentId: 'codex',
      agentLabel: 'Codex',
      canonicalPath: '/tmp/codex',
      displayName: 'Codex CLI',
      compactPath: '~/.codex/skills'
    },
    {
      agentId: 'codex',
      agentLabel: 'Codex',
      canonicalPath: '/tmp/project',
      displayName: 'Project',
      compactPath: '~/project/.agents/skills'
    }
  ];

  assert.deepEqual(usageRankingAgentOptions(hooks), [
    { id: 'claude-code', label: 'Claude Code CLI' },
    { id: 'codex', label: 'Codex App / Codex CLI' }
  ]);
  assert.deepEqual(usageRankingWorkspaceOptions(workspaces), [
    { id: '/tmp/codex', label: 'Codex CLI', detail: '~/.codex/skills' },
    { id: '/tmp/project', label: 'Project', detail: '~/project/.agents/skills' }
  ]);
});
