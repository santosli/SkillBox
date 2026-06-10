export function normalizeSkill(skill) {
  const sourceRoot = skill.sourceRoot || skill.source_root;
  const isSymlink = skill.isSymlink || skill.is_symlink;
  const type = skill.type || inferType(sourceRoot);
  const usageCountValue = Number(skill.usageCount ?? skill.usage_count);

  return {
    ...skill,
    sourceRoot,
    contentHash: skill.contentHash || skill.content_hash,
    skillMdPath: skill.skillMdPath || skill.skill_md_path,
    isSymlink,
    type,
    usageCount: Number.isFinite(usageCountValue) && usageCountValue > 0 ? usageCountValue : 0,
    lastUsedAt: skill.lastUsedAt || skill.last_used_at || '',
    status: skill.status || defaultSkillStatus(type)
  };
}

export function normalizeRemoteSkillVersions(result = {}) {
  const versions = (result.versions || []).map((version) => ({
    version: version.version || '',
    isCurrent: Boolean(version.isCurrent ?? version.is_current),
    kind: version.kind || '',
    shortLabel: version.shortLabel || version.short_label || version.version || '',
    updatedAt: version.updatedAt || version.updated_at || '',
    message: version.message || '',
    path: version.path || ''
  }));

  return {
    skillName: result.skillName || result.skill_name || '',
    currentVersion: result.currentVersion || result.current_version || '',
    versions
  };
}

export function normalizeOperationRecords(result = {}) {
  return (result.operations || []).map((operation) => ({
    id: operation.id || '',
    operationType: operation.type || operation.operationType || operation.operation_type || '',
    status: operation.status || '',
    summary: operation.summary || '',
    error: operation.error || '',
    startedAt: operation.startedAt || operation.started_at || '',
    finishedAt: operation.finishedAt || operation.finished_at || ''
  }));
}

export function mergeSkills(current, imported) {
  const next = new Map(current.map((skill) => [skill.name, skill]));
  for (const skill of imported) {
    next.set(skill.name, skill);
  }
  return Array.from(next.values()).sort((left, right) => left.name.localeCompare(right.name));
}

export function normalizePaths(paths) {
  if (!paths) return paths;

  return {
    ...paths,
    userSkillsRoot: paths.userSkillsRoot || paths.user_skills_root,
    remoteSkillsRoot: paths.remoteSkillsRoot || paths.remote_skills_root,
    databasePath: paths.databasePath || paths.database_path
  };
}

function inferType(sourceRoot = '') {
  if (String(sourceRoot).includes('.agents')) return 'user';
  return 'remote';
}

export function defaultSkillStatus(type) {
  return type === 'user' ? 'sync not checked' : 'update not checked';
}

export function hasAvailableUpdate(skill) {
  const normalized = String(skill.status || '').toLowerCase();
  return skill.type === 'remote' && (normalized.includes('update available') || normalized.includes('new version'));
}

export function labelize(value = '') {
  return String(value).replace(/[-_]/g, ' ');
}

export function compactPath(value = '') {
  return String(value || 'Not available').replace(/^\/Users\/[^/]+(?=\/|$)/, '~');
}

export function joinPath(root, child) {
  if (!root) return child;
  return `${String(root).replace(/\/$/, '')}/${child}`;
}

export function numberOrZero(value) {
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? number : 0;
}
