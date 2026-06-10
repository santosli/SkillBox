import { previewUsageHooks } from './previewData.js';

export function normalizeUsageHookStatuses(rows) {
  const incoming = Array.isArray(rows) ? rows : [];
  const byTarget = new Map();

  for (const row of incoming) {
    const target = row.target || '';
    if (!target) {
      continue;
    }
    byTarget.set(target, {
      target,
      label: row.label || usageHookTargetLabel(target),
      configPath: row.configPath || row.config_path || '',
      command: row.command || '',
      installed: Boolean(row.installed),
      trustRequired: Boolean(row.trustRequired ?? row.trust_required),
      activationNote: row.activationNote || row.activation_note || '',
      sharedConfigKey: row.sharedConfigKey || row.shared_config_key || target
    });
  }

  return previewUsageHooks.map((fallback) => ({
    ...fallback,
    ...(byTarget.get(fallback.target) || {})
  }));
}

export function groupUsageHooksByConfig(hooks) {
  const groups = [];
  const byKey = new Map();

  for (const hook of hooks) {
    const key =
      hook.configPath && hook.command
        ? `${hook.configPath}:${hook.command}`
        : hook.sharedConfigKey || hook.target;
    const existing = byKey.get(key);
    if (existing) {
      existing.labels.push(hook.label);
      existing.installed = existing.installed || hook.installed;
      existing.trustRequired = existing.trustRequired || hook.trustRequired;
      existing.activationNote = existing.activationNote || hook.activationNote;
      continue;
    }

    const group = {
      key,
      target: hook.target,
      labels: [hook.label],
      label: hook.label,
      configPath: hook.configPath,
      command: hook.command,
      installed: hook.installed,
      trustRequired: hook.trustRequired,
      activationNote: hook.activationNote
    };
    byKey.set(key, group);
    groups.push(group);
  }

  return groups.map((group) => ({
    ...group,
    label: group.labels.join(' / ')
  }));
}

export function usageHookBadgeTone(group) {
  if (!group.installed || group.trustRequired) return 'amber';
  return 'green';
}

export function usageHookStatusLabel(group) {
  if (!group.installed) return 'Not injected';
  if (group.trustRequired) return 'Needs trust';
  return 'Injected';
}

function usageHookTargetLabel(target) {
  return previewUsageHooks.find((hook) => hook.target === target)?.label || 'Agent';
}
