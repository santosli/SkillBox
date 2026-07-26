import { compactPath, numberOrZero } from './skills.js';

export const defaultUsageRankingFilters = {
  range: 'last_30_days',
  skillType: '',
  agentId: '',
  workspaceRoot: ''
};

export const usageRankingRangeOptions = [
  { id: 'last_7_days', label: '7 days' },
  { id: 'last_30_days', label: '30 days' },
  { id: 'all_time', label: 'All time' }
];

export const usageRankingSkillTypeOptions = [
  { id: 'user', label: 'User' },
  { id: 'remote', label: 'Remote' },
  { id: 'system', label: 'System' }
];

export function usageRankingRangeLabel(rangeId = defaultUsageRankingFilters.range) {
  return usageRankingRangeOptions.find((option) => option.id === rangeId)?.label
    || usageRankingRangeOptions[1].label;
}

export function usageRankingKindTone(row = {}) {
  if (row.system || row.sourceKind === 'unknown') return 'slate';
  if (row.sourceMissing) return 'red';
  if (!row.managed) return 'amber';
  return String(row.kind || '').trim().toLowerCase() === 'user' ? 'blue' : 'slate';
}

export function usageRankingScopeLabel(row = {}) {
  if (row.system) return 'System';
  if (row.sourceKind === 'unknown') return 'Unknown source';
  if (row.sourceMissing) return 'Deleted';
  if (!row.managed) return 'Not imported';
  const kind = String(row.kind || '').trim().toLowerCase();
  if (kind === 'user') return 'User';
  if (kind === 'remote') return 'Remote';
  return kind ? String(row.kind) : 'Managed';
}

export function formatUsageRankingRank(rank = 0) {
  return `#${String(numberOrZero(rank) || 0).padStart(2, '0')}`;
}

export function usageRankingTopRows(rows = [], limit = 3) {
  return (rows || [])
    .filter((row) => numberOrZero(row.usageCount) > 0)
    .slice(0, Math.max(limit, 0));
}

export function usageRankingBarPercent(usageCount = 0, maxUsageCount = 0) {
  const count = numberOrZero(usageCount);
  const max = numberOrZero(maxUsageCount);
  if (count <= 0 || max <= 0) return 0;
  return Math.max(12, Math.round((count / max) * 100));
}

export function normalizeUsageRankings(result = {}) {
  const coverage = result?.coverage || {};
  const rows = (result?.rows || []).map((row, index) => ({
    rank: numberOrZero(row.rank) || index + 1,
    skillName: row.skillName || row.skill_name || '',
    kind: row.kind || '',
    managed: Boolean(row.managed),
    system: Boolean(row.system),
    sourceMissing: Boolean(row.sourceMissing ?? row.source_missing),
    sourceKind: row.sourceKind || row.source_kind || (row.system ? 'system' : 'regular'),
    sourceId: row.sourceId || row.source_id || '',
    sourceRuntimeRoots: row.sourceRuntimeRoots || row.source_runtime_roots || [],
    usageCount: numberOrZero(row.usageCount ?? row.usage_count),
    lastUsedAt: row.lastUsedAt || row.last_used_at || '',
    confirmedCount: numberOrZero(row.confirmedCount ?? row.confirmed_count),
    inferredCount: numberOrZero(row.inferredCount ?? row.inferred_count),
    referenceCount: numberOrZero(row.referenceCount ?? row.reference_count),
    lastReferencedAt: row.lastReferencedAt || row.last_referenced_at || ''
  }));

  return {
    generatedAt: result?.generatedAt || result?.generated_at || '',
    range: result?.range || defaultUsageRankingFilters.range,
    rangeStart: result?.rangeStart || result?.range_start || '',
    rangeEnd: result?.rangeEnd || result?.range_end || '',
    agentId: result?.agentId || result?.agent_id || '',
    skillType: result?.skillType || result?.skill_type || '',
    workspaceRoot: result?.workspaceRoot || result?.workspace_root || '',
    totalObservedCalls: numberOrZero(
      result?.totalCalls ?? result?.total_calls
        ?? result?.totalObservedCalls ?? result?.total_observed_calls
    ),
    totalCalls: numberOrZero(
      result?.totalCalls ?? result?.total_calls
        ?? result?.totalObservedCalls ?? result?.total_observed_calls
    ),
    totalConfirmedCalls: numberOrZero(
      result?.totalConfirmedCalls ?? result?.total_confirmed_calls
    ),
    totalInferredCalls: numberOrZero(
      result?.totalInferredCalls ?? result?.total_inferred_calls
    ),
    totalHistoryReferences: numberOrZero(
      result?.totalHistoryReferences ?? result?.total_history_references
    ),
    coverage: {
      earliestEventAt: coverage.earliestEventAt || coverage.earliest_event_at || '',
      latestEventAt: coverage.latestEventAt || coverage.latest_event_at || '',
      earliestConfirmedAt: coverage.earliestConfirmedAt || coverage.earliest_confirmed_at || '',
      latestConfirmedAt: coverage.latestConfirmedAt || coverage.latest_confirmed_at || '',
      earliestInferredAt: coverage.earliestInferredAt || coverage.earliest_inferred_at || '',
      latestInferredAt: coverage.latestInferredAt || coverage.latest_inferred_at || '',
      earliestReferenceAt: coverage.earliestReferenceAt || coverage.earliest_reference_at || '',
      latestReferenceAt: coverage.latestReferenceAt || coverage.latest_reference_at || '',
      confirmedCalls: numberOrZero(coverage.confirmedCalls ?? coverage.confirmed_calls),
      inferredCalls: numberOrZero(coverage.inferredCalls ?? coverage.inferred_calls),
      historyReferences: numberOrZero(
        coverage.historyReferences ?? coverage.history_references
      ),
      sourceCounts: (coverage.sourceCounts || coverage.source_counts || []).map((source) => ({
        source: source.source || '',
        evidenceClass: source.evidenceClass || source.evidence_class || 'reference',
        count: numberOrZero(source.count)
      })),
      agentHookCalls: numberOrZero(coverage.agentHookCalls ?? coverage.agent_hook_calls),
      codexSessionBackfillCalls: numberOrZero(
        coverage.codexSessionBackfillCalls ?? coverage.codex_session_backfill_calls
      ),
      claudeCodeSessionBackfillCalls: numberOrZero(
        coverage.claudeCodeSessionBackfillCalls
          ?? coverage.claude_code_session_backfill_calls
      ),
      cursorSessionBackfillCalls: numberOrZero(
        coverage.cursorSessionBackfillCalls ?? coverage.cursor_session_backfill_calls
      ),
      otherObservedCalls: numberOrZero(
        coverage.otherObservedCalls ?? coverage.other_observed_calls
      ),
      scannedCodexSessionFiles: numberOrZero(
        coverage.scannedCodexSessionFiles ?? coverage.scanned_codex_session_files
      ),
      scannedCodexTurns: numberOrZero(
        coverage.scannedCodexTurns ?? coverage.scanned_codex_turns
      ),
      scannedClaudeCodeSessionFiles: numberOrZero(
        coverage.scannedClaudeCodeSessionFiles
          ?? coverage.scanned_claude_code_session_files
      ),
      scannedCursorSessions: numberOrZero(
        coverage.scannedCursorSessions ?? coverage.scanned_cursor_sessions
      ),
      scannedCursorTranscriptFiles: numberOrZero(
        coverage.scannedCursorTranscriptFiles ?? coverage.scanned_cursor_transcript_files
      )
    },
    rows
  };
}

export function usageRankingRequest(filters = defaultUsageRankingFilters) {
  return {
    range: filters.range || defaultUsageRankingFilters.range,
    skillType: filters.skillType || null,
    agentId: filters.agentId || null,
    workspaceRoot: filters.workspaceRoot || null,
    includeUnmanaged: true
  };
}

export function usageRankingAgentOptions(hooks = []) {
  const byId = new Map();

  hooks.forEach((hook) => {
    const id = String(hook.sharedConfigKey || hook.target || '').trim();
    if (!id) return;
    const labels = byId.get(id) || new Set();
    labels.add(hook.label || id);
    byId.set(id, labels);
  });
  if (!byId.has('cursor')) {
    byId.set('cursor', new Set(['Cursor']));
  }

  return [...byId.entries()]
    .map(([id, labels]) => ({ id, label: [...labels].join(' / ') }))
    .sort((left, right) => left.label.localeCompare(right.label));
}

export function usageRankingWorkspaceOptions(workspaces = []) {
  return workspaces
    .map((workspace) => ({
      id: workspace.canonicalPath || workspace.path || '',
      label: workspace.displayName || compactPath(workspace.path) || 'Workspace',
      detail: workspace.compactPath || compactPath(workspace.path)
    }))
    .filter((workspace) => workspace.id)
    .sort((left, right) => left.label.localeCompare(right.label));
}

export function normalizeCodexUsageBackfill(result = {}) {
  return {
    scannedFiles: numberOrZero(result.scannedFiles ?? result.scanned_files),
    scannedTurns: numberOrZero(result.scannedTurns ?? result.scanned_turns),
    discovered: numberOrZero(result.discovered),
    recorded: numberOrZero(result.recorded),
    deduplicated: numberOrZero(result.deduplicated),
    upgraded: numberOrZero(result.upgraded),
    skipped: numberOrZero(result.skipped),
    scannedCursorStateSessions: numberOrZero(
      result.scannedCursorStateSessions ?? result.scanned_cursor_state_sessions
    ),
    cursorStateReferences: numberOrZero(
      result.cursorStateReferences ?? result.cursor_state_references
    ),
    scannedCursorTranscriptFiles: numberOrZero(
      result.scannedCursorTranscriptFiles ?? result.scanned_cursor_transcript_files
    ),
    confirmedCursorTranscriptReads: numberOrZero(
      result.confirmedCursorTranscriptReads ?? result.confirmed_cursor_transcript_reads
    ),
    errors: Array.isArray(result.errors) ? result.errors.map(String) : []
  };
}

export const usageHistorySyncProviders = [
  {
    id: 'codex',
    label: 'Codex',
    command: 'backfill_codex_session_usage',
    request: { includeArchived: true }
  },
  {
    id: 'claude-code',
    label: 'Claude Code',
    command: 'backfill_claude_code_session_usage',
    request: {}
  },
  {
    id: 'cursor',
    label: 'Cursor',
    command: 'backfill_cursor_session_usage',
    request: {}
  }
];

export function usageHistorySyncNotice(results = []) {
  const normalizedResults = results.map((result) => ({
    provider: result.provider || 'History',
    ...normalizeCodexUsageBackfill(result)
  }));
  const scanned = normalizedResults.reduce((total, result) => total + result.scannedFiles, 0);
  const recorded = normalizedResults.reduce((total, result) => total + result.recorded, 0);
  const deduplicated = normalizedResults.reduce(
    (total, result) => total + result.deduplicated,
    0
  );
  const upgraded = normalizedResults.reduce((total, result) => total + result.upgraded, 0);
  const providerSummary = normalizedResults
    .map((result) => {
      const errorLabel = result.errors.length > 0
        ? ` (${result.errors.length} error${result.errors.length === 1 ? '' : 's'})`
        : '';
      const cursorDetail = result.provider === 'Cursor'
        ? `; scanned ${result.scannedCursorTranscriptFiles} transcript files and ${result.scannedCursorStateSessions} state sessions; ${result.confirmedCursorTranscriptReads} confirmed transcript reads, ${result.cursorStateReferences} state references`
        : '';
      return `${result.provider} ${result.recorded} new${cursorDetail}${errorLabel}`;
    })
    .join(', ');
  const parts = [
    `Scanned ${scanned} local history sources`,
    `recorded ${recorded} new observations`,
    `${deduplicated} already recorded`,
    `${upgraded} evidence upgrade${upgraded === 1 ? '' : 's'}`
  ];
  if (providerSummary) {
    parts.push(`by provider: ${providerSummary}`);
  }
  return `${parts.join(', ')}.`;
}

export function codexUsageBackfillNotice(result = {}) {
  const summary = normalizeCodexUsageBackfill(result);
  const parts = [
    `Scanned ${summary.scannedFiles} Codex session files`,
    `recorded ${summary.recorded} new local observations`,
    `${summary.deduplicated} already recorded`
  ];
  if (summary.skipped > 0) {
    parts.push(`${summary.skipped} skipped`);
  }
  if (summary.errors.length > 0) {
    const firstError = summary.errors[0].trim();
    parts.push(
      `${summary.errors.length} error${summary.errors.length === 1 ? '' : 's'}${
        firstError ? ` (${firstError})` : ''
      }`
    );
  }
  return `${parts.join(', ')}.`;
}
