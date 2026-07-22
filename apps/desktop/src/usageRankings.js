import { compactPath, numberOrZero } from './skills.js';

export const defaultUsageRankingFilters = {
  range: 'last_30_days',
  agentId: '',
  workspaceRoot: ''
};

export const usageRankingRangeOptions = [
  { id: 'last_7_days', label: '7 days' },
  { id: 'last_30_days', label: '30 days' },
  { id: 'all_time', label: 'All time' }
];

export function normalizeUsageRankings(result = {}) {
  const rows = (result?.rows || []).map((row, index) => ({
    rank: numberOrZero(row.rank) || index + 1,
    skillName: row.skillName || row.skill_name || '',
    kind: row.kind || '',
    managed: Boolean(row.managed),
    usageCount: numberOrZero(row.usageCount ?? row.usage_count),
    lastUsedAt: row.lastUsedAt || row.last_used_at || ''
  }));

  return {
    generatedAt: result?.generatedAt || result?.generated_at || '',
    range: result?.range || defaultUsageRankingFilters.range,
    rangeStart: result?.rangeStart || result?.range_start || '',
    rangeEnd: result?.rangeEnd || result?.range_end || '',
    agentId: result?.agentId || result?.agent_id || '',
    workspaceRoot: result?.workspaceRoot || result?.workspace_root || '',
    totalObservedCalls: numberOrZero(
      result?.totalObservedCalls ?? result?.total_observed_calls
    ),
    rows
  };
}

export function usageRankingRequest(filters = defaultUsageRankingFilters) {
  return {
    range: filters.range || defaultUsageRankingFilters.range,
    agentId: filters.agentId || null,
    workspaceRoot: filters.workspaceRoot || null,
    includeUnmanaged: false
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
