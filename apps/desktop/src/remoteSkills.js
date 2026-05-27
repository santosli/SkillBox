export function formatRemoteRefBehavior(source = {}) {
  const ref = source.reference || source.ref || '';
  const refKind = source.refKind || source.ref_kind || '';

  if (source.tracking || refKind === 'branch') {
    return `Tracking branch: ${ref || 'main'}`;
  }
  if (refKind === 'tag') {
    return `Pinned tag: ${ref}`;
  }
  return `Pinned commit: ${ref}`;
}

export function normalizeRemoteSourceBindingPreview(preview = {}) {
  const validation = preview.validation || 'mismatch';
  return {
    skillName: preview.skillName || preview.skill_name || '',
    validation,
    currentVersion: preview.currentVersion || preview.current_version || '',
    latestSha: preview.latestSha || preview.latest_sha || '',
    refKind: preview.refKind || preview.ref_kind || '',
    tracking: Boolean(preview.tracking),
    message: preview.message || '',
    replacesCurrent: false,
    statusLabel:
      validation === 'exact_match'
        ? 'Source can be linked; current version already matches.'
        : validation === 'same_skill_changed'
          ? 'Source can be linked; current version will stay active.'
          : 'This source does not match the selected skill.'
  };
}

export function normalizeRemoteVersionPreview(preview = {}) {
  const files = (preview.files || [])
    .map((file) => ({
      path: file.path || '',
      oldPath: file.oldPath || file.old_path || '',
      status: file.status || '',
      label: file.label || remoteFileStatusLabel(file.status || ''),
      diff: file.diff || '',
      oldHash: file.oldHash || file.old_hash || '',
      newHash: file.newHash || file.new_hash || '',
      oldSize: file.oldSize ?? file.old_size ?? null,
      newSize: file.newSize ?? file.new_size ?? null,
      binary: Boolean(file.binary),
      tooLarge: Boolean(file.tooLarge ?? file.too_large)
    }))
    .filter((file) => file.path);

  return {
    previewId: preview.previewId || preview.preview_id || '',
    skillName: preview.skillName || preview.skill_name || '',
    action: preview.action || 'update',
    fromVersion: preview.fromVersion || preview.from_version || '',
    toVersion: preview.toVersion || preview.to_version || '',
    files,
    activePath: files[0]?.path || '',
    affectedDeployments: preview.affectedDeployments || preview.affected_deployments || []
  };
}

export function canApplyRemoteVersionChange({ files = [], loading = false } = {}) {
  return !loading && files.length > 0;
}

export function remoteVersionActionLabel(preview = {}) {
  return preview.action === 'rollback' ? 'Rollback' : 'Update';
}

export function remoteFileStatusLabel(status) {
  if (status.startsWith('A')) return 'Added';
  if (status.startsWith('D')) return 'Deleted';
  if (status.startsWith('R')) return 'Renamed';
  if (status.startsWith('M')) return 'Modified';
  return status || 'Changed';
}
