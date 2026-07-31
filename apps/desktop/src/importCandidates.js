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
  const additionalSourcePaths = [
    ...(candidate.additionalSourcePaths || candidate.additional_source_paths || [])
  ].filter((path, index, paths) => path && path !== sourcePath && paths.indexOf(path) === index);

  return {
    ...candidate,
    sourcePath,
    sourceRoot: candidate.sourceRoot || candidate.source_root,
    realPath,
    isSymlink,
    symlinkTargetPath,
    contentHash: candidate.contentHash || candidate.content_hash,
    additionalSourcePaths,
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

export function normalizeImportCandidateLocation(location = {}) {
  return {
    sourcePath: location.sourcePath || location.source_path || '',
    sourceRoot: location.sourceRoot || location.source_root || '',
    realPath: location.realPath || location.real_path || '',
    isSymlink: Boolean(location.isSymlink ?? location.is_symlink),
    symlinkTargetPath: location.symlinkTargetPath || location.symlink_target_path || ''
  };
}

export function normalizeImportCandidateGroup(group) {
  const variants = (group.variants || []).map((variant, index) => {
    const candidate = normalizeImportCandidate(variant.candidate || variant);
    const locations = (variant.locations || []).map(normalizeImportCandidateLocation);
    const locationCopies = locations
      .map((location) => location.sourcePath)
      .filter((path) => path && path !== candidate.sourcePath);
    return {
      id: variant.id || `variant-${index}-${candidate.sourcePath}`,
      candidate: {
        ...candidate,
        additionalSourcePaths: [...new Set([...candidate.additionalSourcePaths, ...locationCopies])]
      },
      locations: locations.length > 0
        ? locations
        : [normalizeImportCandidateLocation(candidate)]
    };
  });
  const selectedVariantId = group.selectedVariantId || group.selected_variant_id || null;
  const selectedVariant = variants.find((variant) => variant.id === selectedVariantId);

  return {
    id: group.id,
    name: group.name || variants[0]?.candidate.name || '',
    description: group.description || variants[0]?.candidate.description || '',
    usageCount: Number(group.usageCount ?? group.usage_count) || 0,
    requiresReview: Boolean(group.requiresReview ?? group.requires_review),
    selectedVariantId,
    variants,
    isSelected: Boolean(selectedVariant?.candidate.isSelected)
  };
}

export function normalizeImportCandidateGroups(groups = [], candidates = []) {
  if (groups.length > 0) {
    return groups.map(normalizeImportCandidateGroup);
  }

  return candidates.map((candidate, index) => {
    const normalized = normalizeImportCandidate(candidate);
    const variantId = `legacy-variant-${index}`;
    return normalizeImportCandidateGroup({
      id: `legacy-group-${index}`,
      name: normalized.name,
      description: normalized.description,
      usageCount: normalized.usageCount,
      selectedVariantId: normalized.isSelected ? variantId : null,
      variants: [{ id: variantId, candidate: normalized }]
    });
  });
}

export function selectedImportCandidate(group) {
  return group.variants.find((variant) => variant.id === group.selectedVariantId)?.candidate || null;
}

export function isSelectableImportCandidateGroup(group) {
  const candidate = selectedImportCandidate(group);
  return Boolean(candidate && candidate.importStatus === 'importable' && !candidate.conflict);
}

export function toggleImportCandidateGroup(groups, groupId) {
  return groups.map((group) => group.id === groupId && isSelectableImportCandidateGroup(group)
    ? { ...group, isSelected: !group.isSelected }
    : group);
}

export function selectImportCandidateVariant(groups, groupId, variantId) {
  return groups.map((group) => {
    if (group.id !== groupId) return group;
    const variant = group.variants.find((candidateVariant) => candidateVariant.id === variantId);
    if (!variant || variant.candidate.importStatus !== 'importable' || variant.candidate.conflict) {
      return group;
    }
    return { ...group, selectedVariantId: variantId, isSelected: true };
  });
}

export function updateImportCandidateGroupType(groups, groupId, skillType) {
  return groups.map((group) => group.id === groupId
    ? {
        ...group,
        variants: group.variants.map((variant) => variant.id === group.selectedVariantId
          ? { ...variant, candidate: { ...variant.candidate, skillType } }
          : variant)
      }
    : group);
}

export function toggleImportCandidateGroupSelection(groups, targetGroups = groups) {
  const targetIds = new Set(targetGroups.map((group) => group.id));
  const selectable = targetGroups.filter(isSelectableImportCandidateGroup);
  const shouldSelectAll = selectable.some((group) => !group.isSelected);
  return groups.map((group) => targetIds.has(group.id) && isSelectableImportCandidateGroup(group)
    ? { ...group, isSelected: shouldSelectAll }
    : group);
}

export function selectedImportCandidates(groups = []) {
  return groups
    .filter((group) => group.isSelected && isSelectableImportCandidateGroup(group))
    .map(selectedImportCandidate);
}

export function importCandidateGroupLocationCount(group) {
  return group.variants.reduce((count, variant) => count + variant.locations.length, 0);
}

export function importCandidateGroupStatus(group) {
  const statuses = new Set(group.variants.map((variant) => variant.candidate.importStatus));
  return {
    importable: statuses.has('importable'),
    imported: statuses.has('imported'),
    system: statuses.has('system'),
    conflict: group.variants.some((variant) => Boolean(variant.candidate.conflict))
  };
}

export function importCandidateGroupTabs(groups = []) {
  return [
    { id: 'all', label: 'All', count: groups.length },
    { id: 'unimported', label: 'Unimported', count: groups.filter((group) => importCandidateGroupStatus(group).importable).length },
    { id: 'imported', label: 'Imported', count: groups.filter((group) => importCandidateGroupStatus(group).imported).length },
    { id: 'system', label: 'System', count: groups.filter((group) => importCandidateGroupStatus(group).system).length }
  ];
}

export function filterImportCandidateGroups(groups = [], activeTab = 'all') {
  if (activeTab === 'all') return groups;
  const key = activeTab === 'unimported' ? 'importable' : activeTab;
  return groups.filter((group) => importCandidateGroupStatus(group)[key]);
}

export function filterImportCandidateGroupsByQuery(groups = [], query = '') {
  const tokens = String(query).trim().toLowerCase().split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return groups;
  return groups.filter((group) => {
    const searchable = [
      group.name,
      group.description,
      ...group.variants.flatMap((variant) => [
        variant.candidate.description,
        variant.candidate.skillType,
        variant.candidate.importStatus,
        variant.candidate.conflict,
        variant.candidate.suggestionReason,
        ...variant.locations.flatMap((location) => [
          location.sourcePath,
          location.sourceRoot,
          location.realPath,
          location.symlinkTargetPath
        ])
      ])
    ].filter(Boolean).join(' ').toLowerCase();
    return tokens.every((token) => searchable.includes(token));
  });
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
      ...(candidate.additionalSourcePaths || []),
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
