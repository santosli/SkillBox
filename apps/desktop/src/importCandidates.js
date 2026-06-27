export function normalizeImportCandidate(candidate) {
  const suggestedType = candidate.suggestedType || candidate.suggested_type || 'user';
  const sourcePath = candidate.sourcePath || candidate.source_path;
  const conflict = candidate.conflict || null;
  const importStatus = candidate.importStatus || candidate.import_status || 'importable';
  const isImportable = importStatus === 'importable' && !conflict;
  const backendSelected = candidate.isSelected ?? candidate.is_selected;
  const usageCountValue = Number(candidate.usageCount ?? candidate.usage_count);
  const isSymlink = Boolean(candidate.isSymlink ?? candidate.is_symlink);
  const realPath = candidate.realPath || candidate.real_path;
  const symlinkTargetPath =
    candidate.symlinkTargetPath || candidate.symlink_target_path || (isSymlink ? realPath : '');

  return {
    ...candidate,
    sourcePath,
    sourceRoot: candidate.sourceRoot || candidate.source_root,
    realPath,
    isSymlink,
    symlinkTargetPath,
    contentHash: candidate.contentHash || candidate.content_hash,
    suggestedType,
    skillType: candidate.skillType || candidate.skill_type || suggestedType,
    suggestionReason: candidate.suggestionReason || candidate.suggestion_reason || 'Needs confirm',
    importOrigin: candidate.importOrigin || candidate.import_origin || 'local-scan',
    importStatus,
    conflict,
    usageCount: Number.isFinite(usageCountValue) && usageCountValue > 0 ? usageCountValue : 0,
    isSelected: isImportable && (backendSelected ?? true)
  };
}

export function workspaceSkillTabs(candidates = []) {
  const symlinkCandidates = candidates.filter(isWorkspaceSymlinkCandidate);

  return [
    { id: 'all', label: 'All', count: candidates.length },
    { id: 'symlink', label: 'Symlink', count: symlinkCandidates.length },
    { id: 'imported', label: 'Imported', count: candidates.filter(isImportedCandidate).length },
    { id: 'system', label: 'System', count: candidates.filter(isSystemCandidate).length }
  ];
}

export function filterWorkspaceSkillCandidates(candidates = [], activeTab = 'all') {
  if (activeTab === 'symlink') {
    return candidates.filter(isWorkspaceSymlinkCandidate);
  }
  if (activeTab === 'imported') {
    return candidates.filter(isImportedCandidate);
  }
  if (activeTab === 'system') {
    return candidates.filter(isSystemCandidate);
  }
  return candidates;
}

function isImportedCandidate(candidate) {
  return candidate.importStatus === 'imported';
}

function isSystemCandidate(candidate) {
  return candidate.importStatus === 'system';
}

function isWorkspaceSymlinkCandidate(candidate) {
  return candidate.isSymlink && !isImportedCandidate(candidate) && !isSystemCandidate(candidate);
}
