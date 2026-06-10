import { numberOrZero } from './skills.js';

export function normalizeHistory(result = {}) {
  const entries = (result?.entries || []).map((entry) => ({
    id: entry.id || '',
    kind: entry.kind || '',
    timestamp: entry.timestamp || '',
    title: entry.title || '',
    subtitle: entry.subtitle || '',
    status: entry.status || '',
    skillName: entry.skillName || entry.skill_name || '',
    agentId: entry.agentId || entry.agent_id || '',
    runtimeRoot: entry.runtimeRoot || entry.runtime_root || '',
    promptExcerpt: entry.promptExcerpt || entry.prompt_excerpt || '',
    operationType: entry.operationType || entry.operation_type || '',
    actor: entry.actor || '',
    entityType: entry.entityType || entry.entity_type || '',
    entityName: entry.entityName || entry.entity_name || '',
    error: entry.error || ''
  }));

  return {
    entries,
    skillUsageCount: numberOrZero(result?.skillUsageCount ?? result?.skill_usage_count),
    operationCount: numberOrZero(result?.operationCount ?? result?.operation_count)
  };
}

export function groupHistoryEntriesByDay(entries = []) {
  const groups = [];
  const groupByKey = new Map();

  entries.forEach((entry) => {
    const key = historyDayKey(entry.timestamp);
    const label = historyDayLabel(entry.timestamp);
    if (!groupByKey.has(key)) {
      const group = { key, label, entries: [] };
      groupByKey.set(key, group);
      groups.push(group);
    }
    groupByKey.get(key).entries.push(entry);
  });

  return groups;
}

function historyDayKey(timestamp = '') {
  const date = historyDate(timestamp);
  if (!date) return 'unknown';

  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

function historyDayLabel(timestamp = '') {
  const date = historyDate(timestamp);
  if (!date) return 'Unknown date';

  const today = historyDayKey(new Date().toISOString());
  const yesterdayDate = new Date();
  yesterdayDate.setDate(yesterdayDate.getDate() - 1);
  const yesterday = historyDayKey(yesterdayDate.toISOString());
  const key = historyDayKey(timestamp);

  if (key === today) return 'Today';
  if (key === yesterday) return 'Yesterday';

  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${date.getFullYear()}-${month}-${day}`;
}

function historyDate(timestamp = '') {
  const value = String(timestamp || '').trim();
  if (!value) return null;

  const milliseconds = /^\d+$/.test(value) ? Number(value) * 1000 : Date.parse(value);
  const date = new Date(milliseconds);
  return Number.isFinite(milliseconds) && !Number.isNaN(date.getTime()) ? date : null;
}

export function operationStatusTone(status = '') {
  if (status === 'succeeded') return 'green';
  if (status === 'failed') return 'red';
  if (status === 'cancelled') return 'amber';
  return 'slate';
}

export function historyRowSubtitle(entry, isUsage) {
  if (isUsage) return '';

  const subtitle = String(entry.subtitle || '').trim();
  if (!subtitle) return '';

  const defaultOperationSubtitle = entry.operationType && entry.actor
    ? `${entry.operationType} by ${entry.actor}`
    : '';
  return subtitle === defaultOperationSubtitle ? '' : subtitle;
}
