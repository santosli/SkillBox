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
    sourceUrl: preview.sourceUrl || preview.source_url || '',
    path: preview.path || '',
    currentVersion: preview.currentVersion || preview.current_version || '',
    latestSha: preview.latestSha || preview.latest_sha || '',
    reference: preview.reference || preview.ref || '',
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

export function normalizeRemoteSourceCandidates(search = {}) {
  const candidates = (search.candidates || [])
    .map((candidate) => {
      const owner = candidate.owner || '';
      const repo = candidate.repo || '';
      const sourceUrl = candidate.sourceUrl || candidate.source_url || '';

      return {
        owner,
        repo,
        repoLabel: [owner, repo].filter(Boolean).join('/'),
        path: candidate.path || '',
        reference: candidate.reference || '',
        sourceUrl,
        repoUrl: candidate.repoUrl || candidate.repo_url || '',
        name: candidate.name || '',
        description: candidate.description || '',
        stars: Number(candidate.stars || 0),
        archived: Boolean(candidate.archived),
        fork: Boolean(candidate.fork),
        updatedAt: candidate.updatedAt || candidate.updated_at || '',
        matchReasons: candidate.matchReasons || candidate.match_reasons || [],
        score: Number(candidate.score || 0)
      };
    })
    .filter((candidate) => candidate.sourceUrl);

  return {
    skillName: search.skillName || search.skill_name || '',
    candidates
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

export function canApplyRemoteVersionChange({ allowNoFileChanges = false, files = [], loading = false } = {}) {
  return !loading && (files.length > 0 || allowNoFileChanges);
}

export function formatOperationTimestamp(timestamp = '') {
  const value = String(timestamp || '').trim();
  if (!value) return '';

  const milliseconds = /^\d+$/.test(value) ? Number(value) * 1000 : Date.parse(value);
  const date = new Date(milliseconds);
  if (!Number.isFinite(milliseconds) || Number.isNaN(date.getTime())) {
    return value;
  }

  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  const hour = String(date.getHours()).padStart(2, '0');
  const minute = String(date.getMinutes()).padStart(2, '0');
  return `${month}-${day} ${hour}:${minute}`;
}

export function remoteSkillUpdateVersionLabel(remoteUpdate = {}, versions = {}) {
  const versionInfo = versions || {};
  const current =
    remoteUpdate.currentVersion ||
    remoteUpdate.current_version ||
    remoteUpdate.installedSha ||
    remoteUpdate.installed_sha ||
    versionInfo.currentVersion ||
    versionInfo.current_version ||
    '';
  const latest = remoteUpdate.latestSha || remoteUpdate.latest_sha || '';

  if (!current) return 'current unknown';
  return latest ? `${current} -> ${latest}` : current;
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
