export const defaultSyncCommitMessage = 'chore(github): sync user skills';

export function normalizeUserSkillsGitStatus(status) {
  const rawStatus = status?.rawStatus || status?.raw_status || '';
  const changedPaths = status?.changedPaths || status?.changed_paths || parseChangedPaths(rawStatus);

  return {
    repoPath: status?.repoPath || status?.repo_path || '',
    remoteUrl: status?.remoteUrl || status?.remote_url || '',
    branch: status?.branch || '',
    dirty: Boolean(status?.dirty),
    rawStatus,
    changedPaths,
    state: status?.state || 'not_configured',
    message: status?.message || status?.lastError || status?.last_error || ''
  };
}

export function normalizeUserSkillsGitChanges(changes) {
  const files = (changes?.files || []).map((file) => ({
    path: file.path || '',
    status: file.status || '',
    label: gitStatusLabel(file.status || ''),
    diff: file.diff || ''
  })).filter((file) => file.path);

  return {
    repoPath: changes?.repoPath || changes?.repo_path || '',
    initialized: Boolean(changes?.initialized),
    branch: changes?.branch || '',
    remoteUrl: changes?.remoteUrl || changes?.remote_url || '',
    files,
    selectedPaths: files.map((file) => file.path),
    activePath: files[0]?.path || ''
  };
}

export function suggestUserSkillsCommitMessage(files = [], selectedPaths = []) {
  const selected = new Set(selectedPaths);
  const changedFiles = files.filter((file) => selected.size === 0 || selected.has(file.path));
  const skills = [...new Set(changedFiles.map((file) => skillNameFromPath(file.path)).filter(Boolean))];
  const labels = new Set(changedFiles.map((file) => file.label));

  if (changedFiles.length === 0 || skills.length === 0) {
    return defaultSyncCommitMessage;
  }

  if (skills.length === 1) {
    const skill = skills[0];
    if (labels.size === 1 && labels.has('Added')) {
      return `feat(github): add ${skill} skill`;
    }
    if (labels.size === 1 && labels.has('Deleted')) {
      return `chore(github): remove ${skill} skill`;
    }
    if (labels.size === 1 && labels.has('Renamed')) {
      return `chore(github): rename ${skill} skill`;
    }
    return `feat(github): update ${skill} skill`;
  }

  if (skills.length === 2) {
    return `chore(github): sync ${skills[0]} and ${skills[1]} skills`;
  }

  return `chore(github): sync ${skills[0]} and ${skills.length - 1} more skills`;
}

export function canCommitUserSkillsChanges({
  files = [],
  loading = false,
  push = true,
  remoteUrl = '',
  selectedPaths = [],
  status = ''
} = {}) {
  if (loading || status === 'syncing' || status === 'preparing_sync') return false;
  if (files.length === 0) return false;
  if (selectedPaths.length === 0) return false;
  if (push && !remoteUrl.trim()) return false;
  return true;
}

export function userSkillsSyncProgressSteps({ push = true, selectedCount = 0 } = {}) {
  const fileLabel = selectedCount === 1 ? '1 file' : `${selectedCount} files`;
  return [
    `Stage ${fileLabel}`,
    'Create Git commit',
    push ? 'Push to origin/main' : 'Skip push'
  ];
}

export function waitForNextPaint() {
  const requestFrame =
    typeof globalThis.requestAnimationFrame === 'function'
      ? globalThis.requestAnimationFrame.bind(globalThis)
      : null;

  if (!requestFrame) {
    return new Promise((resolve) => setTimeout(resolve, 0));
  }

  return new Promise((resolve) => {
    requestFrame(() => {
      requestFrame(resolve);
    });
  });
}

export function userSyncAction(syncStatus, skillType) {
  if (skillType !== 'user') return null;
  if (!syncStatus || syncStatus.state === 'not_configured') return 'Set up sync';
  if (syncStatus.state === 'push_failed') return 'Retry push';
  return 'Sync now';
}

function skillNameFromPath(path) {
  return (path || '').split('/').filter(Boolean)[0] || '';
}

function gitStatusLabel(status) {
  if (status.includes('D')) return 'Deleted';
  if (status.includes('R')) return 'Renamed';
  if (status.includes('A') || status.includes('?')) return 'Added';
  if (status.includes('M')) return 'Modified';
  return 'Changed';
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

  if (syncStatus?.state === 'dirty') {
    const changed = userSkillHasChangedPath(skill.name, syncStatus.changedPaths);
    return changed
      ? { label: 'Needs sync', tone: 'amber' }
      : { label: 'Synced', tone: 'green' };
  }

  return {
    label: userSyncLabel(syncStatus),
    tone: userSyncTone(syncStatus)
  };
}

function userSkillHasChangedPath(skillName, changedPaths = []) {
  const prefix = `${skillName}/`;
  return changedPaths.some((path) => path === skillName || path.startsWith(prefix));
}

function parseChangedPaths(rawStatus) {
  return (rawStatus || '')
    .split('\n')
    .map((line) => line.trimEnd())
    .filter((line) => line && !line.startsWith('##'))
    .map((line) => line.slice(3).trim())
    .map((path) => path.split(' -> ').pop())
    .filter(Boolean);
}

export function syncNotice(syncStatus) {
  if (syncStatus?.state === 'push_failed') return 'Push failed. Local commit was kept.';
  if (syncStatus?.state === 'dirty') return 'Local changes still need sync.';
  if (syncStatus?.state === 'clean') return 'User skills are synced.';
  return 'User skills sync is not configured.';
}
