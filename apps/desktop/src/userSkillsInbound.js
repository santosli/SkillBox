const relationLabels = {
  unknown: 'Unknown',
  synced: 'Synced',
  ahead: 'Ahead',
  behind: 'Behind',
  diverged: 'Diverged',
  remote_only: 'Remote only',
  no_remote_branch: 'No remote branch'
};

export function normalizeUserSkillsInboundStatus(value) {
  const source = value || {};
  return {
    repoPath: source.repo_path || source.repoPath || '',
    branch: source.branch || 'main',
    remoteUrl: source.remote_url || source.remoteUrl || '',
    worktreeState: source.worktree_state || source.worktreeState || 'clean',
    relation: source.relation || 'unknown',
    localSha: source.local_sha || source.localSha || '',
    remoteSha: source.remote_sha || source.remoteSha || '',
    mergeBaseSha: source.merge_base_sha || source.mergeBaseSha || '',
    aheadCount: Number(source.ahead_count ?? source.aheadCount ?? 0),
    behindCount: Number(source.behind_count ?? source.behindCount ?? 0),
    fetchedAt: source.fetched_at || source.fetchedAt || '',
    fetchError: source.fetch_error || source.fetchError || '',
    message: source.message || ''
  };
}

export function normalizeUserSkillsInboundPreview(value) {
  const source = value || {};
  const files = (source.files || []).map((file) => ({
    ...file,
    oldHash: file.old_hash || file.oldHash || null,
    newHash: file.new_hash || file.newHash || null,
    oldSize: file.old_size ?? file.oldSize ?? null,
    newSize: file.new_size ?? file.newSize ?? null,
    binary: Boolean(file.binary),
    tooLarge: Boolean(file.too_large ?? file.tooLarge)
  }));
  const skillChanges = (source.skill_changes || source.skillChanges || []).map((change) => ({
    skillName: change.skill_name || change.skillName || '',
    previousName: change.previous_name || change.previousName || '',
    kind: change.kind || 'updated',
    files: change.files || [],
    affectedDeployments: (change.affected_deployments || change.affectedDeployments || []).map(
      (deployment) => ({
        targetRoot: deployment.target_root || deployment.targetRoot || '',
        targetPath: deployment.target_path || deployment.targetPath || '',
        profileId: deployment.profile_id || deployment.profileId || '',
        profileName: deployment.profile_name || deployment.profileName || '',
        mode: deployment.mode || '',
        state: deployment.state || '',
        message: deployment.message || ''
      })
    )
  }));
  const safetyIssues = (source.safety_issues || source.safetyIssues || []).map((issue) => ({
    code: issue.code || 'inbound_safety',
    message: issue.message || '',
    path: issue.path || '',
    blocking: Boolean(issue.blocking)
  }));
  const rawConflict = source.conflict_analysis || source.conflictAnalysis;
  const conflictAnalysis = rawConflict
    ? {
        localOnlyCommits: Number(rawConflict.local_only_commits ?? rawConflict.localOnlyCommits ?? 0),
        remoteOnlyCommits: Number(
          rawConflict.remote_only_commits ?? rawConflict.remoteOnlyCommits ?? 0
        ),
        bothChangedFiles: rawConflict.both_changed_files || rawConflict.bothChangedFiles || [],
        bothChangedSkills: rawConflict.both_changed_skills || rawConflict.bothChangedSkills || [],
        likelyConflictFiles:
          rawConflict.likely_conflict_files || rawConflict.likelyConflictFiles || []
      }
    : null;

  return {
    previewId: source.preview_id || source.previewId || '',
    status: normalizeUserSkillsInboundStatus(source.status),
    files,
    skillChanges,
    repositoryFiles: source.repository_files || source.repositoryFiles || [],
    safetyIssues,
    conflictAnalysis,
    canApply: Boolean(source.can_apply ?? source.canApply),
    blockedReason: source.blocked_reason || source.blockedReason || ''
  };
}

export function inboundRelationLabel(status) {
  return relationLabels[status?.relation] || 'Unknown';
}

export function inboundRelationTone(status) {
  if (status?.fetchError || status?.relation === 'diverged') return 'red';
  if (status?.worktreeState === 'dirty' || ['behind', 'remote_only'].includes(status?.relation)) {
    return 'amber';
  }
  if (status?.relation === 'synced') return 'green';
  return 'slate';
}

export function canReviewUserSkillsInbound(status, busy = false) {
  return (
    !busy &&
    Boolean(status) &&
    ['behind', 'remote_only', 'diverged'].includes(status.relation) &&
    !status.fetchError
  );
}

export function canApplyUserSkillsInbound(preview, busy = false) {
  return !busy && Boolean(preview?.canApply && preview?.previewId);
}

export function invalidateUserSkillsInboundPreview(preview) {
  return preview
    ? {
        ...preview,
        canApply: false,
        previewId: ''
      }
    : null;
}

export function beginReviewDialogFocus(initialFocus, restoreFocus) {
  initialFocus?.focus();
  return () => {
    if (restoreFocus && restoreFocus.isConnected !== false) {
      restoreFocus.focus();
    }
  };
}

export function handleReviewDialogKeyDown(
  event,
  { dialogElement, onClose, closeDisabled = false }
) {
  if (event.key === 'Escape') {
    event.preventDefault();
    event.stopPropagation();
    if (!closeDisabled) {
      onClose();
    }
    return;
  }

  if (event.key !== 'Tab') return;

  const focusable = Array.from(
    dialogElement?.querySelectorAll(
      'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
    ) || []
  ).filter((element) => element.getAttribute?.('aria-hidden') !== 'true');

  if (focusable.length === 0) {
    event.preventDefault();
    return;
  }

  const first = focusable[0];
  const last = focusable[focusable.length - 1];
  const activeElement = dialogElement?.ownerDocument?.activeElement;
  const activeIsInside = Boolean(activeElement && dialogElement?.contains(activeElement));

  if (event.shiftKey && (!activeIsInside || activeElement === first)) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && (!activeIsInside || activeElement === last)) {
    event.preventDefault();
    first.focus();
  }
}

export function inboundFileLabel(file) {
  const labels = {
    A: 'Added',
    M: 'Modified',
    D: 'Deleted',
    R: 'Renamed'
  };
  return file?.label || labels[file?.status] || file?.status || 'Changed';
}
