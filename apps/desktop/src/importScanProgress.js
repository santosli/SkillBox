const PHASE_LABELS = {
  preparing: 'Preparing local scan',
  'loading usage and index data': 'Loading usage and index data',
  'scanning local roots': 'Scanning local skill roots',
  'validating candidates': 'Validating skill candidates',
  'grouping Git repositories': 'Grouping Git repositories',
  'loading installed source provenance': 'Loading installed source provenance',
  'loading workspace registry': 'Loading workspace registry',
  complete: 'Import scan complete'
};

export function normalizeImportScanProgress(progress = {}) {
  const processed = Number(progress.processed ?? 0);
  const total = progress.total == null ? null : Number(progress.total);
  const uniqueRepositories = Number(
    progress.uniqueRepositories ?? progress.unique_repositories ?? 0
  );

  return {
    phase: String(progress.phase || 'preparing'),
    processed: Number.isFinite(processed) && processed >= 0 ? processed : 0,
    total: Number.isFinite(total) && total >= 0 ? total : null,
    uniqueRepositories: Number.isFinite(uniqueRepositories) && uniqueRepositories >= 0
      ? uniqueRepositories
      : 0
  };
}

export function importScanProgressLabel(progress) {
  const normalized = normalizeImportScanProgress(progress);
  return PHASE_LABELS[normalized.phase] || normalized.phase;
}

export function importScanProgressDetail(progress) {
  const normalized = normalizeImportScanProgress(progress);
  const count = normalized.total == null
    ? ''
    : `${normalized.processed} of ${normalized.total}`;
  const repositories = normalized.uniqueRepositories > 0
    ? `${normalized.uniqueRepositories} ${normalized.uniqueRepositories === 1 ? 'repository' : 'repositories'}`
    : '';

  return [count, repositories].filter(Boolean).join(' · ') || 'Working locally...';
}

export function isImportScanRequestCurrent(requestId, currentRequestId) {
  return requestId === currentRequestId;
}

export function importScanCommandArgs(scanId) {
  return { scanId };
}

export function createImportScanRequestController() {
  let latestRequestId = 0;
  let activeRequestId = 0;

  return {
    begin() {
      if (activeRequestId !== 0) {
        return null;
      }
      latestRequestId += 1;
      activeRequestId = latestRequestId;
      return activeRequestId;
    },
    finish(requestId) {
      if (activeRequestId === requestId) {
        activeRequestId = 0;
      }
    },
    invalidate() {
      latestRequestId += 1;
      activeRequestId = 0;
    },
    isCurrent(requestId) {
      return requestId === latestRequestId;
    }
  };
}

export function createRemoteImportRequestController() {
  let latestRequestId = 0;
  let activeRequestId = 0;

  return {
    begin() {
      if (activeRequestId !== 0) {
        return null;
      }
      latestRequestId += 1;
      activeRequestId = latestRequestId;
      return activeRequestId;
    },
    finish(requestId) {
      if (activeRequestId === requestId) {
        activeRequestId = 0;
      }
    },
    invalidate() {
      latestRequestId += 1;
      activeRequestId = 0;
    },
    isCurrent(requestId) {
      return requestId === latestRequestId;
    }
  };
}

export function browserImportScanOptions(search = '') {
  const params = new URLSearchParams(search);
  const requestedDelay = Number(params.get('import-scan-delay-ms') || 0);
  return {
    delayMs: Number.isFinite(requestedDelay) ? Math.max(0, Math.min(requestedDelay, 2000)) : 0,
    error: params.get('import-scan-error') === '1'
  };
}

export function waitForImportScanDelay(delayMs) {
  if (!delayMs) {
    return Promise.resolve();
  }
  return new Promise((resolve) => window.setTimeout(resolve, delayMs));
}
