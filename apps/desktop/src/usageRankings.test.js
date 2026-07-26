import assert from 'node:assert/strict';
import test from 'node:test';
import { previewUsageRankings } from './previewData.js';

import {
  defaultUsageRankingFilters,
  formatUsageRankingRank,
  normalizeCodexUsageBackfill,
  codexUsageBackfillNotice,
  normalizeUsageRankings,
  usageRankingAgentOptions,
  usageRankingBarPercent,
  usageRankingKindTone,
  usageRankingRangeLabel,
  usageRankingRequest,
  usageRankingSkillTypeOptions,
  usageHistorySyncNotice,
  usageHistorySyncProviders,
  usageRankingScopeLabel,
  usageRankingTopRows,
  usageRankingWorkspaceOptions
} from './usageRankings.js';

test('normalizes ranking rows and snake case result metadata', () => {
  assert.deepEqual(
    normalizeUsageRankings({
      generated_at: '2026-07-22T10:00:00Z',
      range: 'last_30_days',
      range_start: '2026-06-22T10:00:00Z',
      range_end: '2026-07-22T10:00:00Z',
      skill_type: 'remote',
      total_calls: 12,
      total_observed_calls: 12,
      total_confirmed_calls: 5,
      total_inferred_calls: 7,
      total_history_references: 1,
      coverage: {
        earliest_event_at: '2026-07-01T09:00:00Z',
        latest_event_at: '2026-07-21T08:00:00Z',
        confirmed_calls: 5,
        inferred_calls: 7,
        history_references: 1,
        source_counts: [
          { source: 'agent_hook', evidence_class: 'confirmed', count: 5 },
          { source: 'codex_session_backfill', evidence_class: 'inferred', count: 7 }
        ],
        agent_hook_calls: 5,
        codex_session_backfill_calls: 7,
        claude_code_session_backfill_calls: 2,
        cursor_session_backfill_calls: 1,
        other_observed_calls: 0,
        scanned_codex_session_files: 12,
        scanned_codex_turns: 42,
        scanned_claude_code_session_files: 8,
        scanned_cursor_sessions: 4,
        scanned_cursor_transcript_files: 3
      },
      rows: [
        {
          rank: 1,
          skill_name: 'grill-me',
          kind: 'remote',
          managed: true,
          source_kind: 'regular',
          source_id: 'regular:abc123',
          source_runtime_roots: ['/tmp/project/.codex/skills'],
          usage_count: 12,
          last_used_at: '2026-07-21T08:00:00Z',
          confirmed_count: 5,
          inferred_count: 7,
          reference_count: 1,
          last_referenced_at: '2026-07-20T08:00:00Z'
        }
      ]
    }),
    {
      generatedAt: '2026-07-22T10:00:00Z',
      range: 'last_30_days',
      rangeStart: '2026-06-22T10:00:00Z',
      rangeEnd: '2026-07-22T10:00:00Z',
      agentId: '',
      skillType: 'remote',
      workspaceRoot: '',
      totalObservedCalls: 12,
      totalCalls: 12,
      totalConfirmedCalls: 5,
      totalInferredCalls: 7,
      totalHistoryReferences: 1,
      coverage: {
        earliestEventAt: '2026-07-01T09:00:00Z',
        latestEventAt: '2026-07-21T08:00:00Z',
        earliestConfirmedAt: '',
        latestConfirmedAt: '',
        earliestInferredAt: '',
        latestInferredAt: '',
        earliestReferenceAt: '',
        latestReferenceAt: '',
        confirmedCalls: 5,
        inferredCalls: 7,
        historyReferences: 1,
        sourceCounts: [
          { source: 'agent_hook', evidenceClass: 'confirmed', count: 5 },
          { source: 'codex_session_backfill', evidenceClass: 'inferred', count: 7 }
        ],
        agentHookCalls: 5,
        codexSessionBackfillCalls: 7,
        claudeCodeSessionBackfillCalls: 2,
        cursorSessionBackfillCalls: 1,
        otherObservedCalls: 0,
        scannedCodexSessionFiles: 12,
        scannedCodexTurns: 42,
        scannedClaudeCodeSessionFiles: 8,
        scannedCursorSessions: 4,
        scannedCursorTranscriptFiles: 3
      },
      rows: [
        {
          rank: 1,
          skillName: 'grill-me',
          kind: 'remote',
          managed: true,
          system: false,
          sourceMissing: false,
          sourceKind: 'regular',
          sourceId: 'regular:abc123',
          sourceRuntimeRoots: ['/tmp/project/.codex/skills'],
          usageCount: 12,
          lastUsedAt: '2026-07-21T08:00:00Z',
          confirmedCount: 5,
          inferredCount: 7,
          referenceCount: 1,
          lastReferencedAt: '2026-07-20T08:00:00Z'
        }
      ]
    }
  );
});

test('builds local ranking requests including unmanaged observed skills', () => {
  assert.deepEqual(usageRankingRequest(defaultUsageRankingFilters), {
    range: 'last_30_days',
    skillType: null,
    agentId: null,
    workspaceRoot: null,
    includeUnmanaged: true
  });
  assert.deepEqual(usageRankingSkillTypeOptions.map((option) => option.id), [
    'user',
    'remote',
    'system'
  ]);
  assert.deepEqual(
    usageRankingRequest({
      ...defaultUsageRankingFilters,
      skillType: 'system'
    }),
    {
      range: 'last_30_days',
      skillType: 'system',
      agentId: null,
      workspaceRoot: null,
      includeUnmanaged: true
    }
  );
});

test('filters preview rankings by user, remote, and system skill types', () => {
  const remote = previewUsageRankings({
    ...defaultUsageRankingFilters,
    skillType: 'remote'
  });
  assert.deepEqual(remote.rows.map((row) => row.kind), ['remote']);
  assert.equal(remote.rows[0].rank, 1);
  assert.equal(remote.total_observed_calls, 1);

  const system = previewUsageRankings({
    ...defaultUsageRankingFilters,
    skillType: 'system'
  });
  assert.deepEqual(system.rows.map((row) => row.system), [true]);
  assert.equal(system.rows[0].rank, 1);
  assert.equal(system.total_observed_calls, 1);

  const user = previewUsageRankings({
    ...defaultUsageRankingFilters,
    skillType: 'user'
  });
  assert.equal(user.rows.every((row) => row.kind === 'user'), true);
  assert.equal(user.total_observed_calls, 7);
});

test('formats ranking display helpers for top cards and labels', () => {
  assert.equal(usageRankingRangeLabel('last_7_days'), '7 days');
  assert.equal(usageRankingRangeLabel('last_30_days'), '30 days');
  assert.equal(usageRankingRangeLabel('unknown'), '30 days');
  assert.equal(usageRankingKindTone({ managed: true, kind: 'user' }), 'blue');
  assert.equal(usageRankingKindTone({ managed: true, kind: 'remote' }), 'slate');
  assert.equal(usageRankingKindTone({ managed: false }), 'amber');
  assert.equal(usageRankingKindTone({ managed: false, system: true }), 'slate');
  assert.equal(usageRankingKindTone({ managed: false, sourceKind: 'unknown' }), 'slate');
  assert.equal(usageRankingKindTone({ managed: false, sourceMissing: true }), 'red');
  assert.equal(usageRankingScopeLabel({ managed: true, kind: 'user' }), 'User');
  assert.equal(usageRankingScopeLabel({ managed: true, kind: 'remote' }), 'Remote');
  assert.equal(usageRankingScopeLabel({ managed: false }), 'Not imported');
  assert.equal(usageRankingScopeLabel({ managed: false, system: true }), 'System');
  assert.equal(
    usageRankingScopeLabel({ managed: false, sourceKind: 'unknown' }),
    'Unknown source'
  );
  assert.equal(usageRankingScopeLabel({ managed: false, sourceMissing: true }), 'Deleted');
  assert.equal(formatUsageRankingRank(1), '#01');
  assert.equal(formatUsageRankingRank(12), '#12');
  assert.equal(usageRankingBarPercent(2, 4), 50);
  assert.equal(usageRankingBarPercent(0, 4), 0);
  assert.equal(usageRankingBarPercent(1, 10), 12);
  assert.deepEqual(
    usageRankingTopRows([
      { skillName: 'a', usageCount: 4 },
      { skillName: 'b', usageCount: 0 },
      { skillName: 'c', usageCount: 2 },
      { skillName: 'd', usageCount: 1 },
      { skillName: 'e', usageCount: 9 }
    ]),
    [
      { skillName: 'a', usageCount: 4 },
      { skillName: 'c', usageCount: 2 },
      { skillName: 'd', usageCount: 1 }
    ]
  );
});

test('summarizes Codex usage backfill results', () => {
  assert.deepEqual(
    normalizeCodexUsageBackfill({
      scanned_files: 12,
      discovered: 40,
      recorded: 28,
      deduplicated: 10,
      upgraded: 0,
      skipped: 2,
      scannedCursorStateSessions: 0,
      cursorStateReferences: 0,
      scannedCursorTranscriptFiles: 0,
      confirmedCursorTranscriptReads: 0,
      errors: ['probe: bad path']
    }),
    {
      scannedFiles: 12,
      scannedTurns: 0,
      discovered: 40,
      recorded: 28,
      deduplicated: 10,
      upgraded: 0,
      skipped: 2,
      scannedCursorStateSessions: 0,
      cursorStateReferences: 0,
      scannedCursorTranscriptFiles: 0,
      confirmedCursorTranscriptReads: 0,
      errors: ['probe: bad path']
    }
  );
  assert.equal(
    codexUsageBackfillNotice({
      scannedFiles: 12,
      scannedTurns: 0,
      recorded: 28,
      deduplicated: 10,
      skipped: 2
    }),
    'Scanned 12 Codex session files, recorded 28 new local observations, 10 already recorded, 2 skipped.'
  );
  assert.equal(
    codexUsageBackfillNotice({
      scannedFiles: 12,
      recorded: 28,
      deduplicated: 10,
      errors: ['probe: bad path']
    }),
    'Scanned 12 Codex session files, recorded 28 new local observations, 10 already recorded, 1 error (probe: bad path).'
  );
});

test('syncs Codex, Claude Code, and Cursor histories with one provider-aware notice', () => {
  assert.deepEqual(
    usageHistorySyncProviders.map((provider) => provider.id),
    ['codex', 'claude-code', 'cursor']
  );
  assert.equal(
    usageHistorySyncNotice([
      { provider: 'Codex', scanned_files: 2, recorded: 3, deduplicated: 1 },
      { provider: 'Claude Code', scanned_files: 4, recorded: 2, deduplicated: 0 },
      {
        provider: 'Cursor',
        scanned_files: 5,
        recorded: 1,
        deduplicated: 2,
        upgraded: 1,
        scanned_cursor_state_sessions: 3,
        cursor_state_references: 2,
        scanned_cursor_transcript_files: 2,
        confirmed_cursor_transcript_reads: 4,
        errors: ['unsupported record']
      }
    ]),
    'Scanned 11 local history sources, recorded 6 new observations, 3 already recorded, 1 evidence upgrade, by provider: Codex 3 new, Claude Code 2 new, Cursor 1 new; scanned 2 transcript files and 3 state sessions; 4 confirmed transcript reads, 2 state references (1 error).'
  );
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
    { id: 'codex', label: 'Codex App / Codex CLI' },
    { id: 'cursor', label: 'Cursor' }
  ]);
  assert.deepEqual(usageRankingWorkspaceOptions(workspaces), [
    { id: '/tmp/codex', label: 'Codex CLI', detail: '~/.codex/skills' },
    { id: '/tmp/project', label: 'Project', detail: '~/project/.agents/skills' }
  ]);
});
