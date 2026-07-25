import { previewContentHash } from './previewData.js';

export function remoteImportCandidate(mode, value) {
  const name = inferSkillNameFromImportValue(value);
  const isMarkdown = mode === 'markdown';

  return {
    name,
    description: isMarkdown ? 'Remote skill created from a Markdown file.' : 'Remote skill source provided by URL.',
    sourcePath: value,
    sourceRoot: inferImportSourceRoot(value),
    contentHash: previewContentHash(value),
    suggestedType: 'remote',
    skillType: 'remote',
    suggestionReason: isMarkdown ? 'User provided Markdown file' : 'User provided skill URL',
    importOrigin: 'remote-input',
    importStatus: 'importable',
    isSelected: true,
    conflict: null
  };
}

export function shouldConfirmLocalImport(candidates) {
  return candidates.some((candidate) => isImportableCandidate(candidate) && requiresLocalImportConfirmation(candidate));
}

function requiresLocalImportConfirmation(candidate) {
  const sourcePath = String(candidate.sourcePath || '');

  if (candidate.importOrigin === 'remote-input') {
    return false;
  }

  if (isHttpUrl(sourcePath) || sourcePath.toLowerCase().endsWith('.md')) {
    return false;
  }

  return true;
}

export function importNotice(prefix, message) {
  return [prefix, message].filter(Boolean).join(' ');
}

export function candidateImportSourcePaths(candidate) {
  return [candidate.sourcePath, ...(candidate.additionalSourcePaths || [])].filter(
    (path, index, paths) => path && paths.indexOf(path) === index
  );
}

export function importRequestItems(candidates) {
  return candidates.map((candidate) => ({
    source_path: candidate.sourcePath,
    skill_type: candidate.skillType,
    deploy_back_to_source: true
  }));
}

export function importBatchNotice(result) {
  const imported = result.imported || [];
  const errorCount = (result.errors || []).length;
  const skillCount = new Set(imported.map((item) => item.name).filter(Boolean)).size;
  const locationCount = imported.length;
  const skillLabel = `${skillCount} ${skillCount === 1 ? 'skill' : 'skills'}`;
  const locationSuffix = locationCount > skillCount
    ? ` across ${locationCount} locations`
    : '';
  const shownErrors = (result.errors || []).slice(0, 3).map((item) => {
    const sourcePath = item.sourcePath || item.source_path || 'unknown source';
    return `${sourcePath}: ${item.error || 'Import failed'}`;
  });
  const hiddenErrorCount = Math.max(errorCount - shownErrors.length, 0);
  const errorSuffix = errorCount > 0
    ? ` Failed: ${shownErrors.join('; ')}${hiddenErrorCount > 0 ? `; +${hiddenErrorCount} more` : ''}.`
    : '';

  return `Imported ${skillLabel}${locationSuffix}.${errorSuffix}`.trim();
}

export function isImportableCandidate(candidate) {
  return candidate.importStatus === 'importable' && !candidate.conflict;
}

export function candidateRowClass(candidate) {
  return [
    'candidateRow',
    candidate.conflict ? 'conflict' : '',
    candidate.importStatus === 'imported' ? 'imported' : '',
    candidate.importStatus === 'system' ? 'system' : ''
  ]
    .filter(Boolean)
    .join(' ');
}

export function candidateStatusNote(candidate) {
  if (candidate.conflict) {
    return candidate.conflict;
  }
  if (candidate.importStatus === 'imported' || candidate.importStatus === 'system') {
    return '';
  }
  if (candidateSource(candidate)) {
    return '';
  }
  return candidate.suggestionReason;
}

export function candidateSource(candidate) {
  const values = [
    candidate.sourceRoot,
    candidate.sourcePath,
    candidate.realPath,
    candidate.suggestionReason
  ]
    .filter(Boolean)
    .map((value) => String(value));
  const combined = values.join(' ');

  if (combined.includes('/.agents/skills') || combined.includes('~/.agents/skills')) {
    return { kind: 'agent', label: 'From ~/.agents/skills' };
  }

  if (combined.includes('/.codex/skills') || combined.includes('~/.codex/skills')) {
    return { kind: 'codex', label: 'From ~/.codex/skills' };
  }

  return null;
}

export function toggleImportCandidateSelection(candidates, targetCandidates = candidates) {
  const targetPaths = new Set(targetCandidates.map((candidate) => candidate.sourcePath));
  const selectable = targetCandidates.filter(isImportableCandidate);
  const shouldSelectAll = selectable.some((candidate) => !candidate.isSelected);

  return candidates.map((candidate) =>
    targetPaths.has(candidate.sourcePath) && isImportableCandidate(candidate)
      ? { ...candidate, isSelected: shouldSelectAll }
      : candidate
  );
}

export function isHttpUrl(value) {
  try {
    const parsed = new URL(value);
    return parsed.protocol === 'http:' || parsed.protocol === 'https:';
  } catch {
    return false;
  }
}

function inferSkillNameFromImportValue(value) {
  const clean = value.split(/[?#]/)[0].replace(/\/+$/, '');
  const parts = clean.split(/[\\/]/).filter(Boolean);
  let name = parts[parts.length - 1] || 'remote-skill';

  if (name.toLowerCase() === 'skill.md' && parts.length > 1) {
    name = parts[parts.length - 2];
  } else if (name.toLowerCase().endsWith('.md')) {
    name = name.slice(0, -3);
  }

  return name || 'remote-skill';
}

function inferImportSourceRoot(value) {
  try {
    const parsed = new URL(value);
    const pathParts = parsed.pathname.split('/').filter(Boolean).slice(0, 2);
    return [parsed.hostname, ...pathParts].join('/');
  } catch {
    const clean = value.split(/[?#]/)[0].replace(/\/+$/, '');
    const parts = clean.split(/[\\/]/).filter(Boolean);
    return parts.slice(0, -1).join('/') || clean;
  }
}
