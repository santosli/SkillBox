export const defaultStatusRefreshIntervalMinutes = 5;
export const minStatusRefreshIntervalMinutes = 1;
export const maxStatusRefreshIntervalMinutes = 1440;
export const defaultRemoteUpdateTimeoutSeconds = 30;
export const minRemoteUpdateTimeoutSeconds = 5;
export const maxRemoteUpdateTimeoutSeconds = 300;
export const statusNoticeAutoCloseSeconds = 8;

export function formatStatusNoticeCountdown(seconds) {
  const remaining = Math.max(0, Math.ceil(Number(seconds) || 0));

  if (remaining === 0) return 'Closing...';
  return `Closes in ${remaining}s`;
}

export function normalizeStatusRefreshIntervalMinutes(value) {
  const minutes = Number(value);

  if (
    Number.isInteger(minutes) &&
    minutes >= minStatusRefreshIntervalMinutes &&
    minutes <= maxStatusRefreshIntervalMinutes
  ) {
    return minutes;
  }

  return defaultStatusRefreshIntervalMinutes;
}

export function normalizeRemoteUpdateTimeoutSeconds(value) {
  const seconds = Number(value);

  if (
    Number.isInteger(seconds) &&
    seconds >= minRemoteUpdateTimeoutSeconds &&
    seconds <= maxRemoteUpdateTimeoutSeconds
  ) {
    return seconds;
  }

  return defaultRemoteUpdateTimeoutSeconds;
}

export function formatStatusCheckedAt(checkedAt, now = new Date()) {
  if (!checkedAt) return 'not checked';

  const checkedValue = String(checkedAt).trim();
  const checkedDate = /^\d+$/.test(checkedValue)
    ? new Date(Number(checkedValue) * 1000)
    : new Date(checkedValue);
  if (Number.isNaN(checkedDate.getTime())) return 'not checked';

  const checkedDay = [
    checkedDate.getFullYear(),
    checkedDate.getMonth(),
    checkedDate.getDate()
  ].join('-');
  const currentDay = [now.getFullYear(), now.getMonth(), now.getDate()].join('-');
  const time = [
    checkedDate.getHours(),
    checkedDate.getMinutes(),
    checkedDate.getSeconds()
  ]
    .map((part) => String(part).padStart(2, '0'))
    .join(':');

  if (checkedDay === currentDay) {
    return time;
  }

  const date = [
    checkedDate.getFullYear(),
    String(checkedDate.getMonth() + 1).padStart(2, '0'),
    String(checkedDate.getDate()).padStart(2, '0')
  ].join('-');

  return `${date} ${time.slice(0, 5)}`;
}

export function normalizeRemoteSkillUpdates(result) {
  const statuses = (result?.statuses || []).map((status) => ({
    skillName: status.skillName || status.skill_name || '',
    sourceType: status.sourceType || status.source_type || '',
    sourceUrl: status.sourceUrl || status.source_url || status.url || '',
    currentVersion: status.currentVersion || status.current_version || '',
    installedSha: status.installedSha || status.installed_sha || '',
    latestSha: status.latestSha || status.latest_sha || '',
    refKind: status.refKind || status.ref_kind || '',
    tracking: Boolean(status.tracking),
    updateAvailable: Boolean(status.updateAvailable ?? status.update_available),
    state: status.state || 'not_checkable',
    stateLabel: remoteUpdateStateLabel(status.state || 'not_checkable'),
    message: status.message || ''
  }));

  return {
    checkedAt: result?.checkedAt || result?.checked_at || '',
    statuses
  };
}

function remoteUpdateStateLabel(state) {
  if (state === 'no_source') return 'No source';
  if (state === 'update_available') return 'Update available';
  if (state === 'up_to_date') return 'Up to date';
  if (state === 'pinned') return 'Pinned';
  if (state === 'check_failed') return 'Check failed';
  return 'Not checkable';
}

export function remoteSkillRowStatus(skill, remoteUpdates) {
  if (skill?.type !== 'remote') return null;
  const status = remoteUpdates?.statuses?.find((item) => item.skillName === skill.name);
  if (!status) return null;

  if (status.state === 'no_source') {
    return { label: 'No source', tone: 'slate' };
  }
  if (status.state === 'update_available') {
    return { label: 'Update available', tone: 'amber' };
  }
  if (status.state === 'up_to_date') {
    return { label: 'Up to date', tone: 'green' };
  }
  if (status.state === 'pinned') {
    return { label: 'Pinned', tone: 'blue' };
  }
  if (status.state === 'check_failed') {
    return { label: 'Check failed', tone: 'red' };
  }
  return { label: 'Not checkable', tone: 'slate' };
}

export function dashboardStatusNotice({ userSkillsGit, remoteUpdates }) {
  const statuses = remoteUpdates?.statuses || [];
  const updates = statuses.filter((status) => status.state === 'update_available').length;
  const upToDate = statuses.filter((status) => status.state === 'up_to_date').length;
  const pinned = statuses.filter((status) => status.state === 'pinned').length;
  const failed = statuses.filter((status) => status.state === 'check_failed').length;
  const noSource = statuses.filter((status) => status.state === 'no_source').length;
  const notCheckable = statuses.filter((status) => status.state === 'not_checkable').length;
  const parts = [];

  if (updates) parts.push(`${updates} remote ${updates === 1 ? 'update' : 'updates'} available`);
  if (upToDate) parts.push(`${upToDate} up to date`);
  if (pinned) parts.push(`${pinned} pinned`);
  if (failed) parts.push(`${failed} check ${failed === 1 ? 'failed' : 'failures'}`);
  if (noSource) parts.push(`${noSource} missing ${noSource === 1 ? 'source' : 'sources'}`);
  if (notCheckable) parts.push(`${notCheckable} not checkable`);

  if (userSkillsGit?.state === 'dirty') {
    parts.push('user skills need sync');
  } else if (userSkillsGit?.state === 'clean') {
    parts.push('user skills synced');
  } else if (userSkillsGit?.state === 'push_failed' || userSkillsGit?.state === 'error') {
    parts.push('user skills check failed');
  } else {
    parts.push('user skills sync not configured');
  }

  return `${parts.join(', ')}.`;
}
