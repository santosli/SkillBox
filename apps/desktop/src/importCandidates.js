export function normalizeImportCandidate(candidate) {
  const suggestedType = candidate.suggestedType || candidate.suggested_type || 'user';
  const sourcePath = candidate.sourcePath || candidate.source_path;
  const conflict = candidate.conflict || null;
  const importStatus = candidate.importStatus || candidate.import_status || 'importable';
  const isImportable = importStatus === 'importable' && !conflict;
  const backendSelected = candidate.isSelected ?? candidate.is_selected;

  return {
    ...candidate,
    sourcePath,
    sourceRoot: candidate.sourceRoot || candidate.source_root,
    realPath: candidate.realPath || candidate.real_path,
    contentHash: candidate.contentHash || candidate.content_hash,
    suggestedType,
    skillType: candidate.skillType || candidate.skill_type || suggestedType,
    suggestionReason: candidate.suggestionReason || candidate.suggestion_reason || 'Needs confirm',
    importOrigin: candidate.importOrigin || candidate.import_origin || 'local-scan',
    importStatus,
    conflict,
    isSelected: isImportable && (backendSelected ?? true)
  };
}
