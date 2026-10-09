const PROVIDER_LABELS = {
  codex: 'Codex',
  'claude-code': 'Claude Code',
  cursor: 'Cursor'
};

export function normalizeUsageBackfillProgress(progress = {}) {
  const processed = Number(progress.processed ?? 0);
  const total = progress.total == null ? null : Number(progress.total);
  const providerIndex = Number(progress.providerIndex ?? progress.provider_index ?? 0);
  const providerCount = Number(progress.providerCount ?? progress.provider_count ?? 0);

  return {
    provider: String(progress.provider || ''),
    phase: String(progress.phase || 'starting'),
    processed: Number.isFinite(processed) && processed >= 0 ? processed : 0,
    total: Number.isFinite(total) && total >= 0 ? total : null,
    providerIndex: Number.isFinite(providerIndex) && providerIndex > 0 ? providerIndex : 0,
    providerCount: Number.isFinite(providerCount) && providerCount > 0 ? providerCount : 0
  };
}

export function usageBackfillProgressLabel(progress) {
  const normalized = normalizeUsageBackfillProgress(progress);
  const provider = PROVIDER_LABELS[normalized.provider] || '';
  if (normalized.phase === 'scanning-state') {
    return 'Scanning Cursor sessions';
  }
  if (normalized.phase === 'scanning-transcripts') {
    return 'Scanning Cursor transcripts';
  }
  if (normalized.phase === 'collecting') {
    return provider ? `Finding ${provider} histories` : 'Finding local histories';
  }
  if (normalized.phase === 'complete') {
    return provider ? `Finished ${provider}` : 'Finished scanning';
  }
  return provider ? `Scanning ${provider} histories` : 'Scanning local agent histories';
}

export function usageBackfillProgressDetail(progress) {
  const normalized = normalizeUsageBackfillProgress(progress);
  const count = countLabel(normalized);
  const agents = normalized.providerIndex > 0 && normalized.providerCount > 0
    ? `${normalized.providerIndex} of ${normalized.providerCount} agents`
    : '';

  return [count, agents].filter(Boolean).join(' · ') || 'Working locally...';
}

export function usageBackfillProgressPercent(progress) {
  const normalized = normalizeUsageBackfillProgress(progress);
  if (normalized.total == null || normalized.total <= 0) {
    return null;
  }
  return Math.min(100, Math.round((normalized.processed / normalized.total) * 100));
}

function countLabel(progress) {
  if (progress.total == null) {
    return '';
  }
  const unit = unitLabel(progress.phase, progress.total);
  return `${progress.processed} of ${progress.total} ${unit}`;
}

function unitLabel(phase, total) {
  if (phase === 'scanning-state') {
    return total === 1 ? 'session' : 'sessions';
  }
  if (phase === 'scanning-transcripts') {
    return total === 1 ? 'transcript' : 'transcripts';
  }
  return total === 1 ? 'file' : 'files';
}
