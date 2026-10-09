import { compactPath, numberOrZero } from './skills.js';

export const defaultUsageRankingFilters = {
  range: 'all_time',
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
    || usageRankingRangeOptions.find((option) => option.id === defaultUsageRankingFilters.range)?.label
    || 'All time';
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

export const usageHeatmapDays = 365;
export const usageHeatmapWeekStartsOn = 0;
export const usageHeatmapLevelColors = [
  'var(--skillbox-border-subtle)',
  'rgba(var(--skillbox-blue-rgb), 0.22)',
  'rgba(var(--skillbox-blue-rgb), 0.40)',
  'rgba(var(--skillbox-blue-rgb), 0.62)',
  'var(--skillbox-blue)'
];

export function usageTrendSeriesKey(skill = {}) {
  return `${skill.sourceKind || skill.source_kind || 'regular'}:${skill.skillName || skill.skill_name || ''}`;
}

export function usageTrendSeriesLabel(skill = {}) {
  const name = skill.skillName || skill.skill_name || '';
  const sourceKind = skill.sourceKind || skill.source_kind || 'regular';
  if (sourceKind === 'system') return `${name} · System`;
  if (sourceKind === 'unknown') return `${name} · Unknown`;
  return name;
}

export function formatUsageTrendDate(date, { compact = false, includeYear = false } = {}) {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(date || ''));
  if (!match) return String(date || '');
  const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
  const monthLabel = months[Number(match[2]) - 1] || match[2];
  const day = String(Number(match[3]));
  if (compact) return `${Number(match[2])}/${day}`;
  if (includeYear) return `${monthLabel} ${day}, ${match[1]}`;
  return `${monthLabel} ${day}`;
}

function usageHeatmapUtcDate(value) {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(value || ''));
  if (!match) return null;
  return new Date(`${match[1]}-${match[2]}-${match[3]}T00:00:00Z`);
}

function usageHeatmapIsoDate(value) {
  if (!(value instanceof Date) || Number.isNaN(value.getTime())) return '';
  return value.toISOString().slice(0, 10);
}

function usageHeatmapShiftUtcDate(value, days) {
  const next = new Date(value.getTime());
  next.setUTCDate(next.getUTCDate() + days);
  return next;
}

export function usageHeatmapLevel(calls = 0, maxCalls = 0) {
  const value = numberOrZero(calls);
  const max = numberOrZero(maxCalls);
  if (value <= 0 || max <= 0) return 0;
  if (value >= max) return 4;
  const ratio = value / max;
  if (ratio > 0.6) return 4;
  if (ratio > 0.35) return 3;
  if (ratio > 0.2) return 2;
  return 1;
}

export function usageHeatmapLevelColor(level = 0) {
  return usageHeatmapLevelColors[Math.max(0, Math.min(4, numberOrZero(level)))]
    || usageHeatmapLevelColors[0];
}

export const usageHeatmapCellSize = 18;
export const usageHeatmapGap = 2;
export const usageHeatmapWeekdayWidth = 28;
export const usageHeatmapWeekdayGap = 4;
export const usageHeatmapMonthLabelMinWidth = 24;
export const usageHeatmapWeekdayLabels = [
  { label: 'Mon', weekday: 1 },
  { label: 'Wed', weekday: 3 },
  { label: 'Fri', weekday: 5 }
];

export function usageHeatmapWeekdayRows(weekStartsOn = usageHeatmapWeekStartsOn) {
  const start = ((numberOrZero(weekStartsOn) % 7) + 7) % 7;
  return usageHeatmapWeekdayLabels.map((item) => ({
    label: item.label,
    weekday: item.weekday,
    row: (item.weekday - start + 7) % 7
  }));
}

export function usageHeatmapLayout(weekCount = 0) {
  const weeks = Math.max(numberOrZero(weekCount), 1);
  const cell = usageHeatmapCellSize;
  const gap = usageHeatmapGap;
  const weekdayWidth = usageHeatmapWeekdayWidth;
  const weekdayGap = usageHeatmapWeekdayGap;
  const gridWidth = weeks * cell + (weeks - 1) * gap;
  return {
    cell,
    cellHeight: cell,
    gap,
    weekdayWidth,
    weekdayGap,
    weekCount: weeks,
    gridWidth,
    gridHeight: 7 * cell + 6 * gap,
    canvasWidth: weekdayWidth + weekdayGap + gridWidth
  };
}

export function visibleUsageHeatmapMonths(months = [], layout = {}, weekCount = 0) {
  const list = (Array.isArray(months) ? months : [])
    .map((month) => ({
      label: String(month.label || ''),
      weekIndex: Math.max(numberOrZero(month.weekIndex), 0)
    }))
    .filter((month) => month.label)
    .sort((left, right) => left.weekIndex - right.weekIndex);
  if (list.length === 0) {
    return [];
  }

  const cell = Math.max(numberOrZero(layout.cell), 1);
  const gap = numberOrZero(layout.gap);
  const step = cell + gap;
  const weeks = Math.max(
    numberOrZero(weekCount),
    numberOrZero(layout.weekCount),
    list[list.length - 1].weekIndex + 1
  );
  const minWeeks = Math.max(1, Math.ceil(usageHeatmapMonthLabelMinWidth / step));

  return list.filter((month, index) => {
    const nextIndex = list[index + 1]?.weekIndex ?? weeks;
    return nextIndex - month.weekIndex >= minWeeks;
  });
}

export function buildUsageHeatmap(daily = []) {
  const points = Array.isArray(daily) ? daily : [];
  const byDate = new Map(points.map((point) => [point.date, point]));
  if (points.length === 0) {
    return {
      weeks: [],
      months: [],
      days: [],
      max: 0,
      start: '',
      end: ''
    };
  }

  const start = points[0].date;
  const end = points[points.length - 1].date;
  const startDate = usageHeatmapUtcDate(start);
  const endDate = usageHeatmapUtcDate(end);
  if (!startDate || !endDate || startDate > endDate) {
    return {
      weeks: [],
      months: [],
      days: [],
      max: 0,
      start,
      end
    };
  }

  const alignedStart = usageHeatmapShiftUtcDate(
    startDate,
    -((startDate.getUTCDay() - usageHeatmapWeekStartsOn + 7) % 7)
  );
  const weeks = [];
  const days = [];
  let cursor = alignedStart;
  do {
    const week = [];
    for (let index = 0; index < 7; index += 1) {
      const date = usageHeatmapIsoDate(cursor);
      const inRange = date >= start && date <= end;
      const point = byDate.get(date);
      const cell = {
        date,
        inRange,
        totalCalls: inRange ? numberOrZero(point?.totalCalls) : 0,
        skills: inRange ? (point?.skills || []) : []
      };
      week.push(cell);
      if (inRange) {
        days.push(cell);
      }
      cursor = usageHeatmapShiftUtcDate(cursor, 1);
    }
    weeks.push(week);
  } while (cursor <= endDate);

  const max = days.reduce((highest, day) => Math.max(highest, day.totalCalls), 0);
  for (const day of days) {
    day.level = usageHeatmapLevel(day.totalCalls, max);
  }
  for (const week of weeks) {
    for (const day of week) {
      day.level = day.inRange ? usageHeatmapLevel(day.totalCalls, max) : 0;
    }
  }

  const monthNames = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
  const months = [];
  weeks.forEach((week, weekIndex) => {
    const firstOfMonth = week.find((day) => day.inRange && day.date.endsWith('-01'));
    if (firstOfMonth) {
      months.push({
        label: monthNames[Number(firstOfMonth.date.slice(5, 7)) - 1],
        weekIndex
      });
    }
  });
  if (months.every((month) => month.weekIndex !== 0)) {
    months.unshift({
      label: monthNames[Number(start.slice(5, 7)) - 1],
      weekIndex: 0
    });
  }

  return { weeks, months, days, max, start, end };
}

export function usageStatsTableRows(rows = [], selectedDay = null) {
  const rankingRows = Array.isArray(rows) ? rows : [];
  if (!selectedDay) return rankingRows;
  const byKey = new Map(rankingRows.map((row) => [usageTrendSeriesKey(row), row]));
  return (selectedDay.skills || [])
    .filter((skill) => numberOrZero(skill.calls) > 0)
    .sort((left, right) => (
      numberOrZero(right.calls) - numberOrZero(left.calls)
      || String(left.skillName || left.skill_name || '').localeCompare(
        String(right.skillName || right.skill_name || '')
      )
      || String(left.sourceKind || left.source_kind || '').localeCompare(
        String(right.sourceKind || right.source_kind || '')
      )
    ))
    .map((skill, index) => {
      const matched = byKey.get(usageTrendSeriesKey(skill)) || {};
      const date = String(selectedDay.date || '');
      return {
        rank: index + 1,
        skillName: skill.skillName || skill.skill_name || matched.skillName || '',
        kind: matched.kind || '',
        managed: Boolean(matched.managed),
        system: Boolean(matched.system || skill.sourceKind === 'system'),
        sourceMissing: Boolean(matched.sourceMissing),
        sourceKind: skill.sourceKind || skill.source_kind || matched.sourceKind || 'regular',
        sourceId: matched.sourceId || '',
        sourceRuntimeRoots: matched.sourceRuntimeRoots || [],
        usageCount: numberOrZero(skill.calls),
        lastUsedAt: /^\d{4}-\d{2}-\d{2}$/.test(date) ? `${date}T12:00:00.000Z` : (matched.lastUsedAt || ''),
        confirmedCount: numberOrZero(matched.confirmedCount),
        inferredCount: numberOrZero(matched.inferredCount),
        referenceCount: numberOrZero(matched.referenceCount),
        lastReferencedAt: matched.lastReferencedAt || ''
      };
    });
}

export function normalizeUsageDailyPoints(points = []) {
  return (Array.isArray(points) ? points : []).map((point) => ({
    date: String(point.date || ''),
    totalCalls: numberOrZero(point.totalCalls ?? point.total_calls),
    skills: (point.skills || []).map((skill) => ({
      skillName: skill.skillName || skill.skill_name || '',
      sourceKind: skill.sourceKind || skill.source_kind || 'regular',
      calls: numberOrZero(skill.calls)
    }))
  }));
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
    rows,
    daily: normalizeUsageDailyPoints(result?.daily)
  };
}

export function usageRankingHasSnapshot(ranking = {}) {
  return Boolean(String(ranking?.generatedAt || '').trim());
}

export function usageRankingBodyMode(ranking = {}, { loading = false, backfilling = false } = {}) {
  if (backfilling) return 'progress';
  if (loading && !usageRankingHasSnapshot(ranking)) return 'loading';
  return 'snapshot';
}

export function usageRankingFiltersLocked({
  backfilling = false,
  importingSkillName = ''
} = {}) {
  return Boolean(backfilling || importingSkillName);
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
    inferredCursorTranscriptCalls: numberOrZero(
      result.inferredCursorTranscriptCalls ?? result.inferred_cursor_transcript_calls
    ),
    cursorTranscriptReadCandidates: numberOrZero(
      result.cursorTranscriptReadCandidates ?? result.cursor_transcript_read_candidates
    ),
    cursorTranscriptReadFileCandidates: numberOrZero(
      result.cursorTranscriptReadFileCandidates ?? result.cursor_transcript_read_file_candidates
    ),
    cursorTranscriptTurnDuplicates: numberOrZero(
      result.cursorTranscriptTurnDuplicates ?? result.cursor_transcript_turn_duplicates
    ),
    cursorTranscriptDuplicateFiles: numberOrZero(
      result.cursorTranscriptDuplicateFiles ?? result.cursor_transcript_duplicate_files
    ),
    cursorTranscriptHistoricalMissing: numberOrZero(
      result.cursorTranscriptHistoricalMissing ?? result.cursor_transcript_historical_missing
    ),
    cursorTranscriptUnsafeRejected: numberOrZero(
      result.cursorTranscriptUnsafeRejected ?? result.cursor_transcript_unsafe_rejected
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
        ? `; scanned ${result.scannedCursorTranscriptFiles} transcript files and ${result.scannedCursorStateSessions} state sessions; ${result.inferredCursorTranscriptCalls} inferred transcript calls from ${result.cursorTranscriptReadCandidates} Read candidates and ${result.cursorTranscriptReadFileCandidates} ReadFile candidates, ${result.cursorStateReferences} state references; ${result.cursorTranscriptTurnDuplicates} same-turn duplicates, ${result.cursorTranscriptDuplicateFiles} duplicate files, ${result.cursorTranscriptHistoricalMissing} historical missing paths accepted, ${result.cursorTranscriptUnsafeRejected} unsafe paths rejected`
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
