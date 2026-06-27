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
  const visibleCandidates = visibleImportCandidates(candidates);
  const unimportedCandidates = visibleCandidates.filter(isUnimportedCandidate);

  return [
    { id: 'all', label: 'All', count: visibleCandidates.length },
    { id: 'unimported', label: 'Unimported', count: unimportedCandidates.length },
    { id: 'imported', label: 'Imported', count: visibleCandidates.filter(isImportedCandidate).length },
    { id: 'system', label: 'System', count: visibleCandidates.filter(isSystemCandidate).length }
  ];
}

export function filterWorkspaceSkillCandidates(candidates = [], activeTab = 'all') {
  const visibleCandidates = visibleImportCandidates(candidates);

  if (activeTab === 'unimported') {
    return visibleCandidates.filter(isUnimportedCandidate);
  }
  if (activeTab === 'imported') {
    return visibleCandidates.filter(isImportedCandidate);
  }
  if (activeTab === 'system') {
    return visibleCandidates.filter(isSystemCandidate);
  }
  return visibleCandidates;
}

export function visibleImportCandidates(candidates = []) {
  const sourcePaths = new Set(
    candidates
      .filter((candidate) => !candidate.isSymlink)
      .map((candidate) => normalizedCandidatePath(candidate.realPath || candidate.sourcePath))
      .filter(Boolean)
  );

  return candidates.filter((candidate) => {
    if (!candidate.isSymlink) {
      return true;
    }

    const targetPath = normalizedCandidatePath(candidate.symlinkTargetPath || candidate.realPath);
    return !targetPath || !sourcePaths.has(targetPath);
  });
}

export function filterImportCandidatesByQuery(candidates = [], query = '') {
  const tokens = String(query)
    .trim()
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean);

  if (tokens.length === 0) {
    return candidates;
  }

  return candidates.filter((candidate) => {
    const searchable = [
      candidate.name,
      candidate.description,
      candidate.sourcePath,
      candidate.realPath,
      candidate.symlinkTargetPath,
      candidate.skillType,
      candidate.importStatus
    ]
      .filter(Boolean)
      .join(' ')
      .toLowerCase();

    return tokens.every((token) => searchable.includes(token));
  });
}

function isImportedCandidate(candidate) {
  return candidate.importStatus === 'imported';
}

function isSystemCandidate(candidate) {
  return candidate.importStatus === 'system';
}

function isUnimportedCandidate(candidate) {
  return !isImportedCandidate(candidate) && !isSystemCandidate(candidate);
}

function normalizedCandidatePath(path) {
  return String(path || '').replace(/\/+$/, '');
}
