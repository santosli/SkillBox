export function normalizeRemoteSkillUpdates(result) {
  const statuses = (result?.statuses || []).map((status) => ({
    skillName: status.skillName || status.skill_name || '',
    sourceType: status.sourceType || status.source_type || '',
    installedSha: status.installedSha || status.installed_sha || '',
    latestSha: status.latestSha || status.latest_sha || '',
    updateAvailable: Boolean(status.updateAvailable ?? status.update_available),
    state: status.state || 'not_checkable',
    message: status.message || ''
  }));

  return { statuses };
}

export function remoteSkillRowStatus(skill, remoteUpdates) {
  if (skill?.type !== 'remote') return null;
  const status = remoteUpdates?.statuses?.find((item) => item.skillName === skill.name);
  if (!status) return null;

  if (status.state === 'update_available') {
    return { label: 'Update available', tone: 'amber' };
  }
  if (status.state === 'up_to_date') {
    return { label: 'Up to date', tone: 'green' };
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
  const failed = statuses.filter((status) => status.state === 'check_failed').length;
  const notCheckable = statuses.filter((status) => status.state === 'not_checkable').length;
  const parts = [];

  if (updates) parts.push(`${updates} remote ${updates === 1 ? 'update' : 'updates'} available`);
  if (upToDate) parts.push(`${upToDate} up to date`);
  if (failed) parts.push(`${failed} check ${failed === 1 ? 'failed' : 'failures'}`);
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
