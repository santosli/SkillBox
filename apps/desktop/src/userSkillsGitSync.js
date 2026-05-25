export const defaultSyncCommitMessage = 'Sync user skills';

export function normalizeUserSkillsGitStatus(status) {
  return {
    repoPath: status?.repoPath || status?.repo_path || '',
    remoteUrl: status?.remoteUrl || status?.remote_url || '',
    branch: status?.branch || '',
    dirty: Boolean(status?.dirty),
    rawStatus: status?.rawStatus || status?.raw_status || '',
    state: status?.state || 'not_configured',
    message: status?.message || status?.lastError || status?.last_error || ''
  };
}

export function userSyncAction(syncStatus, skillType) {
  if (skillType !== 'user') return null;
  if (!syncStatus || syncStatus.state === 'not_configured') return 'Set up sync';
  if (syncStatus.state === 'push_failed') return 'Retry push';
  return 'Sync now';
}

export function userSyncTone(syncStatus) {
  if (syncStatus?.state === 'clean') return 'green';
  if (syncStatus?.state === 'push_failed' || syncStatus?.state === 'error') return 'red';
  if (syncStatus?.state === 'dirty' || syncStatus?.state === 'not_configured') return 'amber';
  return 'slate';
}

export function userSyncLabel(syncStatus) {
  if (syncStatus?.state === 'clean') return 'Synced';
  if (syncStatus?.state === 'dirty') return 'Needs sync';
  if (syncStatus?.state === 'push_failed') return 'Push failed';
  if (syncStatus?.state === 'error') return 'Error';
  return 'Not configured';
}

export function userSkillRowStatus(skill, syncStatus) {
  if (skill?.type !== 'user') return null;
  return {
    label: userSyncLabel(syncStatus),
    tone: userSyncTone(syncStatus)
  };
}

export function syncNotice(syncStatus) {
  if (syncStatus?.state === 'push_failed') return 'Push failed. Local commit was kept.';
  if (syncStatus?.state === 'dirty') return 'Local changes still need sync.';
  if (syncStatus?.state === 'clean') return 'User skills are synced.';
  return 'User skills sync is not configured.';
}
