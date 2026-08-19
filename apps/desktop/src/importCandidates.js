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
    symlinkTargetPath: location.symlinkTargetPath || location.symlink_target_path || '',
    suggestedType: location.suggestedType || location.suggested_type || '',
    suggestionReason: location.suggestionReason || location.suggestion_reason || ''
  };
}

export function normalizeImportCandidateGroup(group) {
  const variants = (group.variants || []).map((variant, index) => {
    const candidate = normalizeImportCandidate(variant.candidate || variant);
    const locations = (variant.locations || []).map(normalizeImportCandidateLocation);
    const locationCopies = locations
      .map((location) => location.sourcePath)
      .filter((path) => path && path !== candidate.sourcePath);
    const suggestedTypes = variant.suggestedTypes || variant.suggested_types || [candidate.suggestedType];
    const requiresTypeReview = Boolean(variant.requiresTypeReview ?? variant.requires_type_review);
    const selectedType = variant.selectedType ?? variant.selected_type ?? (
      requiresTypeReview ? null : suggestedTypes[0] || candidate.suggestedType
    );
    return {
      id: variant.id || `variant-${index}-${candidate.sourcePath}`,
      candidate: {
        ...candidate,
        skillType: selectedType || candidate.skillType,
        additionalSourcePaths: [...new Set([...candidate.additionalSourcePaths, ...locationCopies])]
      },
      locations: locations.length > 0
        ? locations
        : [normalizeImportCandidateLocation(candidate)],
      suggestedTypes,
      requiresTypeReview,
      selectedType
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
    isSelected: Boolean(selectedVariant?.candidate.isSelected && selectedVariant.selectedType)
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

export function normalizeImportCollections(collections = []) {
  return collections.map((collection) => ({
    ...collection,
    id: collection.id,
    sourceKind: collection.sourceKind || collection.source_kind || 'git_worktree',
    previewId: collection.previewId || collection.preview_id || '',
    displayName: collection.displayName || collection.display_name || 'Git repository',
    canonicalWorktreeRoot: collection.canonicalWorktreeRoot || collection.canonical_worktree_root || '',
    canonicalRepositoryId: collection.canonicalRepositoryId || collection.canonical_repository_id || '',
    originUrl: collection.originUrl || collection.origin_url || '',
    sourceUrl: collection.sourceUrl || collection.source_url || '',
    requestedReference: collection.requestedReference || collection.requested_reference || '',
    branch: collection.branch || '',
    detached: Boolean(collection.detached),
    reviewedHeadSha: collection.reviewedHeadSha || collection.reviewed_head_sha || '',
    children: (collection.children || []).map((child) => ({
      ...child,
      id: child.id,
      groupId: child.groupId || child.group_id || '',
      variantId: child.variantId || child.variant_id || '',
      name: child.name || '',
      relativePath: child.relativePath || child.relative_path || '',
      sourcePath: child.sourcePath || child.source_path || '',
      realPath: child.realPath || child.real_path || '',
      snapshotHash: child.snapshotHash || child.snapshot_hash || '',
      diff: child.diff || '',
      contentHash: child.contentHash || child.content_hash || '',
      importStatus: child.importStatus || child.import_status || 'importable',
      suggestedTypes: child.suggestedTypes || child.suggested_types || [],
      requiresTypeReview: (child.importStatus || child.import_status || 'importable') === 'importable'
        && !child.conflict
        && Boolean(child.requiresTypeReview ?? child.requires_type_review),
      selectedType: child.selectedType ?? child.selected_type ?? null,
      isSelected: Boolean(child.isSelected ?? child.is_selected),
      locations: (child.locations || []).map(normalizeImportCandidateLocation),
      unlinkedLocations: (child.unlinkedLocations || child.unlinked_locations || [])
        .map(normalizeImportCandidateLocation)
    })),
    errors: collection.errors || []
  }));
}

export function normalizeGithubSkillCollectionPreviewResult(result = {}) {
  if (result.kind === 'collection' && result.preview) {
    return { kind: 'collection', preview: result.preview };
  }
  if (result.kind === 'single_skill' || result.kind === 'explicit_reference_required') {
    return {
      kind: result.kind,
      message: result.message || 'GitHub collection preview could not continue.'
    };
  }
  throw new Error('GitHub collection preview returned an invalid result.');
}

export function importCollectionGroupIds(collections = [], { liveOnly = false } = {}) {
  return new Set(
    collections
      .filter((collection) => !liveOnly || ['git_worktree', 'github_remote'].includes(collection.sourceKind))
      .flatMap((collection) => collection.children.map((child) => child.groupId))
  );
}

export function filterImportCollectionsByQuery(collections = [], query = '') {
  const tokens = String(query).trim().toLowerCase().split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return collections;
  return collections.filter((collection) => {
    const searchable = [
      collection.displayName,
      collection.canonicalWorktreeRoot,
      collection.canonicalRepositoryId,
      collection.originUrl,
      collection.branch,
      collection.reviewedHeadSha,
      ...collection.children.flatMap((child) => [
        child.name,
        child.relativePath,
        child.sourcePath,
        child.realPath,
        child.importStatus,
        child.conflict,
        ...child.locations.flatMap((location) => [location.sourcePath, location.realPath]),
        ...child.unlinkedLocations.flatMap((location) => [location.sourcePath, location.realPath])
      ])
    ].filter(Boolean).join(' ').toLowerCase();
    return tokens.every((token) => searchable.includes(token));
  });
}

export function selectedImportCollectionRequests(groups = [], collections = []) {
  return collections.filter((collection) => ['git_worktree', 'github_remote'].includes(collection.sourceKind)).map((collection) => {
    if (collectionTypeChoiceState(groups, collection).required) return null;
    const selections = collection.children
      .map((child) => {
        const group = groups.find((candidateGroup) => candidateGroup.id === child.groupId);
        if (!group || !group.isSelected || group.selectedVariantId !== child.variantId) return null;
        const variant = group.variants.find((candidateVariant) => candidateVariant.id === child.variantId);
        if (!variant || !variant.selectedType || variant.candidate.conflict || child.conflict) return null;
        return {
          relativePath: child.relativePath,
          groupId: child.groupId,
          variantId: child.variantId,
          skillType: variant.selectedType
        };
      })
      .filter(Boolean);
    return selections.length === 0
      ? null
      : {
          collectionId: collection.id,
          sourceKind: collection.sourceKind,
          sourceUrl: collection.sourceUrl || collection.originUrl || '',
          worktreeRoot: collection.canonicalWorktreeRoot,
          previewId: collection.previewId,
          selections
        };
  }).filter(Boolean);
}

export function selectedImportCandidate(group) {
  const variant = selectedImportCandidateVariant(group);
  return variant
    ? { ...variant.candidate, skillType: variant.selectedType || variant.candidate.skillType }
    : null;
}

export function selectedImportCandidateVariant(group) {
  return group.variants.find((variant) => variant.id === group.selectedVariantId) || null;
}

export function canClassifyImportCandidateGroup(group) {
  const variant = selectedImportCandidateVariant(group);
  return Boolean(
    variant
    && variant.candidate.importStatus === 'importable'
    && !variant.candidate.conflict
  );
}

export function collectionChildTypeState(group, child) {
  const variant = group.variants.find((candidateVariant) => candidateVariant.id === child.variantId);
  const selectedVariant = group.selectedVariantId === child.variantId;
  const importableChild = child.importStatus === 'importable' && !child.conflict;
  const canClassify = importableChild
    && selectedVariant
    && canClassifyImportCandidateGroup({
      ...group,
      selectedVariantId: child.variantId
    });
  const childType = importableChild && selectedVariant && variant
    ? variant.selectedType
    : child.selectedType || (selectedVariant ? variant?.selectedType : null);
  const canSelect = canClassify && Boolean(childType);
  const needsTypeChoice = canClassify && child.requiresTypeReview && !childType;
  const readOnlyLabel = child.importStatus === 'imported' && childType
    ? `Managed as ${childType === 'remote' ? 'Remote' : 'User'}`
    : child.conflict
      ? 'Resolve conflict before import'
      : child.importStatus === 'system'
        ? 'System skill'
        : !selectedVariant
          ? 'Choose a variant first'
          : child.importStatus !== 'importable' && childType
            ? `${childType === 'remote' ? 'Remote' : 'User'} (read only)`
            : '';

  return {
    childType,
    canClassify,
    canSelect,
    importableChild,
    needsTypeChoice,
    readOnlyLabel,
    selectedVariant
  };
}

function collectionActionableGroups(groups = [], collection = {}) {
  const groupById = new Map(groups.map((group) => [group.id, group]));
  const actionable = [];
  const seenGroupIds = new Set();

  for (const child of collection.children || []) {
    const group = groupById.get(child.groupId);
    if (
      !group
      || seenGroupIds.has(group.id)
      || group.selectedVariantId !== child.variantId
      || child.importStatus !== 'importable'
      || child.conflict
      || !canClassifyImportCandidateGroup(group)
    ) {
      continue;
    }
    const variant = selectedImportCandidateVariant(group);
    if (!variant || variant.id !== child.variantId) continue;
    seenGroupIds.add(group.id);
    actionable.push({ group, variant });
  }

  return actionable;
}

export function collectionTypeChoiceState(groups = [], collection = {}) {
  const actionable = collectionActionableGroups(groups, collection);
  const selectedTypes = new Set(
    actionable
      .map(({ variant }) => variant.selectedType)
      .filter((type) => ['user', 'remote'].includes(type))
  );
  const allHaveType = actionable.length > 0
    && actionable.every(({ variant }) => ['user', 'remote'].includes(variant.selectedType));
  const selectedType = allHaveType && selectedTypes.size === 1
    ? [...selectedTypes][0]
    : null;

  return {
    actionableGroupIds: new Set(actionable.map(({ group }) => group.id)),
    actionableCount: actionable.length,
    selectedType,
    required: actionable.length > 0 && !selectedType
  };
}

export function collectionTypeReviewGroupIds(groups = [], collections = []) {
  const groupIds = new Set();
  for (const collection of collections) {
    const typeState = collectionTypeChoiceState(groups, collection);
    if (!typeState.required) continue;
    for (const groupId of typeState.actionableGroupIds) groupIds.add(groupId);
  }
  return groupIds;
}

export function updateImportCollectionType(groups = [], collection = {}, skillType) {
  if (!['user', 'remote'].includes(skillType)) return groups;
  const { actionableGroupIds } = collectionTypeChoiceState(groups, collection);

  return groups.map((group) => actionableGroupIds.has(group.id)
    ? {
        ...group,
        isSelected: true,
        variants: group.variants.map((variant) => variant.id === group.selectedVariantId
          ? {
              ...variant,
              selectedType: skillType,
              candidate: { ...variant.candidate, skillType }
            }
          : variant)
      }
    : group);
}

export function collectionSkillCountLabel(count) {
  const normalized = Math.max(0, Math.trunc(Number(count) || 0));
  return `${normalized} ${normalized === 1 ? 'skill' : 'skills'}`;
}

export function isSelectableImportCandidateGroup(group) {
  const variant = selectedImportCandidateVariant(group);
  return Boolean(canClassifyImportCandidateGroup(group) && variant.selectedType);
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
    return {
      ...group,
      selectedVariantId: variantId,
      isSelected: Boolean(variant.selectedType)
    };
  });
}

export function updateImportCandidateGroupType(groups, groupId, skillType) {
  return groups.map((group) => group.id === groupId
    ? {
        ...group,
        isSelected: canClassifyImportCandidateGroup(group),
        variants: group.variants.map((variant) => variant.id === group.selectedVariantId
          ? {
              ...variant,
              selectedType: skillType,
              candidate: { ...variant.candidate, skillType }
            }
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

export function collectionEligibleGroupIds(groups = [], collection = {}) {
  if (collectionTypeChoiceState(groups, collection).required) return new Set();
  const groupById = new Map(groups.map((group) => [group.id, group]));
  const eligibleIds = new Set();

  for (const child of collection.children || []) {
    const group = groupById.get(child.groupId);
    if (
      !group
      || group.selectedVariantId !== child.variantId
      || child.importStatus !== 'importable'
      || child.conflict
      || !isSelectableImportCandidateGroup(group)
    ) {
      continue;
    }
    eligibleIds.add(group.id);
  }

  return eligibleIds;
}

export function collectionSelectionState(groups = [], collection = {}) {
  const eligibleGroupIds = collectionEligibleGroupIds(groups, collection);
  const eligibleGroups = groups.filter((group) => eligibleGroupIds.has(group.id));
  const selectedCount = eligibleGroups.filter((group) => group.isSelected).length;
  const eligibleCount = eligibleGroups.length;

  return {
    eligibleGroupIds,
    eligibleCount,
    selectedCount,
    allSelected: eligibleCount > 0 && selectedCount === eligibleCount,
    indeterminate: selectedCount > 0 && selectedCount < eligibleCount
  };
}

export function toggleImportCollectionSelection(groups = [], collection = {}) {
  const eligibleGroupIds = collectionEligibleGroupIds(groups, collection);
  return toggleImportCandidateGroupSelection(
    groups,
    groups.filter((group) => eligibleGroupIds.has(group.id))
  );
}

export function importReviewSelectableGroups(groups = [], collections = []) {
  const typeReviewGroupIds = collectionTypeReviewGroupIds(groups, collections);
  return groups.filter((group) => (
    isSelectableImportCandidateGroup(group)
    && !typeReviewGroupIds.has(group.id)
  ));
}

export function toggleImportReviewSelection(groups = [], collections = []) {
  return toggleImportCandidateGroupSelection(
    groups,
    importReviewSelectableGroups(groups, collections)
  );
}

export function selectedImportCandidates(groups = [], collections = []) {
  const liveCollectionGroupIds = importCollectionGroupIds(collections, { liveOnly: true });
  const typeReviewGroupIds = collectionTypeReviewGroupIds(groups, collections);
  return groups
    .filter((group) => (
      group.isSelected
      && isSelectableImportCandidateGroup(group)
      && !liveCollectionGroupIds.has(group.id)
      && !typeReviewGroupIds.has(group.id)
    ))
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
        ...variant.suggestedTypes,
        ...variant.locations.flatMap((location) => [
          location.sourcePath,
          location.sourceRoot,
          location.realPath,
          location.symlinkTargetPath,
          location.suggestedType,
          location.suggestionReason
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
