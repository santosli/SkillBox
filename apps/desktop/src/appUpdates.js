export function normalizeAppUpdateStatus(rawStatus, currentVersion = '') {
  const status = rawStatus || {};
  const checkedAt = status.checkedAt || status.checked_at || '';
  const message = status.message || status.error || '';
  const disabled = Boolean(status.disabled);
  const available = Boolean(status.available);
  const version = status.version || '';
  const normalizedCurrentVersion =
    status.currentVersion || status.current_version || currentVersion || '';

  let state = status.state || 'idle';
  if (disabled) {
    state = 'disabled';
  } else if (status.error) {
    state = 'error';
  } else if (available) {
    state = 'available';
  } else if (checkedAt) {
    state = 'up_to_date';
  }

  return {
    state,
    available,
    currentVersion: normalizedCurrentVersion,
    version,
    date: status.date || '',
    body: status.body || '',
    checkedAt,
    message
  };
}

export function appUpdateNotice(updateStatus) {
  if (updateStatus?.available && updateStatus.version) {
    return `SkillBox v${updateStatus.version} is available.`;
  }
  if (updateStatus?.state === 'up_to_date') {
    return 'SkillBox is up to date.';
  }
  return updateStatus?.message || '';
}

export function shouldCheckAppUpdateOnStartup({ tauriAvailable, updateStatus }) {
  return Boolean(tauriAvailable && updateStatus?.state === 'idle' && !updateStatus.checkedAt);
}
