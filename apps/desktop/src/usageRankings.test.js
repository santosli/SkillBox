import assert from 'node:assert/strict';
import test from 'node:test';
import { previewUsageRankings } from './previewData.js';

import {
  defaultUsageRankingFilters,
  formatUsageRankingRank,
  formatUsageTrendDate,
  buildUsageHeatmap,
  usageHeatmapDays,
  usageHeatmapLayout,
  usageHeatmapCellSize,
  usageHeatmapMonthLabelMinWidth,
  usageHeatmapWeekdayRows,
  visibleUsageHeatmapMonths,
  usageHeatmapLevel,
  usageHeatmapLevelColor,
  usageHeatmapLevelColors,
  usageStatsTableRows,
  normalizeCodexUsageBackfill,
  normalizeUsageDailyPoints,
  codexUsageBackfillNotice,
  normalizeUsageRankings,
  usageRankingAgentOptions,
  usageRankingBodyMode,
  usageRankingFiltersLocked,
  usageRankingHasSnapshot,
  usageRankingKindTone,
  usageRankingRangeLabel,
  usageRankingRequest,
  usageRankingSkillTypeOptions,
  usageHistorySyncNotice,
  usageHistorySyncProviders,
  usageRankingScopeLabel,
  usageRankingWorkspaceOptions
} from './usageRankings.js';
import {
  normalizeUsageBackfillProgress,
  usageBackfillProgressDetail,
  usageBackfillProgressLabel,
  usageBackfillProgressPercent
} from './usageBackfillProgress.js';

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
      daily: [
        {
          date: '2026-07-21',
          total_calls: 12,
          skills: [
            { skill_name: 'grill-me', source_kind: 'regular', calls: 12 }
          ]
        }
      ],
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
      daily: [
        {
          date: '2026-07-21',
          totalCalls: 12,
          skills: [
            { skillName: 'grill-me', sourceKind: 'regular', calls: 12 }
          ]
        }
      ],
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
    range: 'all_time',
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
      range: 'all_time',
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
  assert.equal(
    user.daily.reduce((total, point) => total + point.total_calls, 0),
    7
  );
  assert.equal(
    remote.daily.reduce((total, point) => total + point.total_calls, 0),
    1
  );
  assert.equal(user.daily.length, usageHeatmapDays);
  assert.equal(user.daily[0].date, '2025-07-23');
  assert.equal(user.daily.at(-1).date, '2026-07-22');
});

test('keeps a loaded usage snapshot visible while filters refetch', () => {
  const empty = normalizeUsageRankings(null);
  const loaded = normalizeUsageRankings({
    generatedAt: '2026-09-20T04:00:00Z',
    totalCalls: 12,
    rows: [{ skill_name: 'release-helper', usage_count: 12 }]
  });

  assert.equal(usageRankingHasSnapshot(empty), false);
  assert.equal(usageRankingHasSnapshot(loaded), true);
  assert.equal(usageRankingBodyMode(empty, { loading: true }), 'loading');
  assert.equal(usageRankingBodyMode(loaded, { loading: true }), 'snapshot');
  assert.equal(usageRankingBodyMode(loaded, { loading: true, backfilling: true }), 'progress');
  assert.equal(usageRankingFiltersLocked({ backfilling: false, importingSkillName: '' }), false);
  assert.equal(usageRankingFiltersLocked({ backfilling: true }), true);
  assert.equal(usageRankingFiltersLocked({ importingSkillName: 'release-helper' }), true);
});

test('formats ranking labels and a year call heatmap', () => {
  assert.equal(usageRankingRangeLabel('last_7_days'), '7 days');
  assert.equal(usageRankingRangeLabel('last_30_days'), '30 days');
  assert.equal(usageRankingRangeLabel('unknown'), 'All time');
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
  assert.equal(formatUsageTrendDate('2026-07-22'), 'Jul 22');
  assert.equal(formatUsageTrendDate('2026-07-22', { compact: true }), '7/22');
  assert.equal(formatUsageTrendDate('2026-07-22', { includeYear: true }), 'Jul 22, 2026');
  assert.deepEqual(
    normalizeUsageDailyPoints([
      {
        date: '2026-07-21',
        total_calls: 4,
        skills: [{ skill_name: 'alpha', source_kind: 'system', calls: 4 }]
      }
    ]),
    [
      {
        date: '2026-07-21',
        totalCalls: 4,
        skills: [{ skillName: 'alpha', sourceKind: 'system', calls: 4 }]
      }
    ]
  );

  const heatmap = buildUsageHeatmap([
    {
      date: '2026-07-01',
      totalCalls: 0,
      skills: []
    },
    {
      date: '2026-07-02',
      totalCalls: 5,
      skills: [
        { skillName: 'alpha', sourceKind: 'regular', calls: 3 },
        { skillName: 'beta', sourceKind: 'regular', calls: 2 }
      ]
    },
    {
      date: '2026-07-03',
      totalCalls: 1,
      skills: [{ skillName: 'gamma', sourceKind: 'system', calls: 1 }]
    }
  ]);
  assert.equal(heatmap.start, '2026-07-01');
  assert.equal(heatmap.end, '2026-07-03');
  assert.equal(heatmap.max, 5);
  assert.equal(heatmap.days.length, 3);
  assert.ok(heatmap.weeks.length >= 1);
  assert.equal(heatmap.weeks[0].length, 7);
  assert.equal(heatmap.days[1].level, 4);
  assert.equal(heatmap.days[2].level, 1);
  assert.deepEqual(
    heatmap.days[1].skills.map((skill) => skill.skillName),
    ['alpha', 'beta']
  );
  assert.ok(heatmap.months.some((month) => month.label === 'Jul' && month.weekIndex === 0));
  assert.equal(usageHeatmapLevel(0, 10), 0);
  assert.equal(usageHeatmapLevel(1, 10), 1);
  assert.equal(usageHeatmapLevel(10, 10), 4);
  assert.equal(
    usageHeatmapLevelColors.every((color) => color.includes('skillbox-blue') || color.includes('skillbox-border-subtle')),
    true
  );
  assert.equal(usageHeatmapLevelColor(4), 'var(--skillbox-blue)');
  const layout = usageHeatmapLayout(53);
  assert.equal(layout.cell, usageHeatmapCellSize);
  assert.equal(layout.cellHeight, layout.cell);
  assert.equal(layout.gridHeight, 7 * layout.cell + 6 * layout.gap);
  assert.equal(layout.gridWidth, 53 * layout.cell + 52 * layout.gap);
  assert.equal(layout.canvasWidth, layout.weekdayWidth + layout.weekdayGap + layout.gridWidth);
  assert.deepEqual(
    usageHeatmapWeekdayRows().map((row) => `${row.label}:${row.row}`),
    ['Mon:1', 'Wed:3', 'Fri:5']
  );

  const overlappingMonths = [
    { label: 'Sep', weekIndex: 0 },
    { label: 'Oct', weekIndex: 1 },
    { label: 'Nov', weekIndex: 5 },
    { label: 'Dec', weekIndex: 10 }
  ];
  const visibleMonths = visibleUsageHeatmapMonths(overlappingMonths, layout, 53);
  assert.equal(usageHeatmapMonthLabelMinWidth, 24);
  assert.deepEqual(visibleMonths.map((month) => month.label), ['Oct', 'Nov', 'Dec']);
  for (let index = 1; index < visibleMonths.length; index += 1) {
    const px = (visibleMonths[index].weekIndex - visibleMonths[index - 1].weekIndex)
      * (layout.cell + layout.gap);
    assert.ok(px >= usageHeatmapMonthLabelMinWidth);
  }
  assert.deepEqual(
    visibleUsageHeatmapMonths(overlappingMonths, { cell: 24, gap: 2 }, 53)
      .map((month) => month.label),
    ['Sep', 'Oct', 'Nov', 'Dec']
  );

  const rankingRows = [
    {
      rank: 1,
      skillName: 'nightwatch-video',
      managed: true,
      sourceKind: 'regular',
      sourceId: 'nightwatch',
      usageCount: 189,
      lastUsedAt: '2026-09-20T03:29:00Z'
    },
    {
      rank: 2,
      skillName: 'lark-wiki-obsidian-sync',
      managed: true,
      sourceKind: 'regular',
      sourceId: 'lark',
      usageCount: 12,
      lastUsedAt: '2026-09-01T00:00:00Z'
    }
  ];
  assert.equal(usageStatsTableRows(rankingRows, null), rankingRows);
  const dayRows = usageStatsTableRows(rankingRows, {
    date: '2026-06-01',
    totalCalls: 2,
    skills: [
      { skillName: 'lark-wiki-obsidian-sync', sourceKind: 'regular', calls: 1 },
      { skillName: 'personal-wiki-updater', sourceKind: 'regular', calls: 1 }
    ]
  });
  assert.deepEqual(dayRows.map((row) => [row.rank, row.skillName, row.usageCount, row.managed, row.sourceId]), [
    [1, 'lark-wiki-obsidian-sync', 1, true, 'lark'],
    [2, 'personal-wiki-updater', 1, false, '']
  ]);
  assert.equal(dayRows[0].lastUsedAt, '2026-06-01T12:00:00.000Z');
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
      inferredCursorTranscriptCalls: 0,
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
      inferredCursorTranscriptCalls: 0,
      cursorTranscriptReadCandidates: 0,
      cursorTranscriptReadFileCandidates: 0,
      cursorTranscriptTurnDuplicates: 0,
      cursorTranscriptDuplicateFiles: 0,
      cursorTranscriptHistoricalMissing: 0,
      cursorTranscriptUnsafeRejected: 0,
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
        inferred_cursor_transcript_calls: 4,
        cursor_transcript_read_candidates: 7,
        cursor_transcript_read_file_candidates: 1,
        cursor_transcript_turn_duplicates: 2,
        cursor_transcript_duplicate_files: 1,
        cursor_transcript_historical_missing: 3,
        cursor_transcript_unsafe_rejected: 1,
        errors: ['unsupported record']
      }
    ]),
    'Scanned 11 local history sources, recorded 6 new observations, 3 already recorded, 1 evidence upgrade, by provider: Codex 3 new, Claude Code 2 new, Cursor 1 new; scanned 2 transcript files and 3 state sessions; 4 inferred transcript calls from 7 Read candidates and 1 ReadFile candidates, 2 state references; 2 same-turn duplicates, 1 duplicate files, 3 historical missing paths accepted, 1 unsafe paths rejected (1 error).'
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

test('formats usage history scan progress for each provider', () => {
  assert.deepEqual(
    normalizeUsageBackfillProgress({
      provider: 'codex',
      phase: 'scanning',
      processed: 42,
      total: 466,
      provider_index: 1,
      provider_count: 3
    }),
    {
      provider: 'codex',
      phase: 'scanning',
      processed: 42,
      total: 466,
      providerIndex: 1,
      providerCount: 3
    }
  );
  assert.equal(
    usageBackfillProgressLabel({ provider: 'codex', phase: 'scanning' }),
    'Scanning Codex histories'
  );
  assert.equal(
    usageBackfillProgressDetail({
      provider: 'codex',
      phase: 'scanning',
      processed: 42,
      total: 466,
      providerIndex: 1,
      providerCount: 3
    }),
    '42 of 466 files · 1 of 3 agents'
  );
  assert.equal(
    usageBackfillProgressPercent({ processed: 42, total: 466 }),
    9
  );
  assert.equal(
    usageBackfillProgressLabel({ provider: 'cursor', phase: 'scanning-transcripts' }),
    'Scanning Cursor transcripts'
  );
  assert.equal(
    usageBackfillProgressDetail({
      phase: 'scanning-transcripts',
      processed: 1,
      total: 1,
      providerIndex: 3,
      providerCount: 3
    }),
    '1 of 1 transcript · 3 of 3 agents'
  );
  assert.equal(usageBackfillProgressPercent({ phase: 'collecting' }), null);
  assert.equal(
    usageBackfillProgressLabel({}),
    'Scanning local agent histories'
  );
  assert.equal(usageBackfillProgressDetail({}), 'Working locally...');
});
