import React from 'react';
import { RefreshCw, X } from 'lucide-react';
import { GitDiffView } from '../GitDiffView.jsx';
import { closeOnBackdropClick } from '../modalEvents.js';
import {
  canApplyRemoteVersionChange,
  formatRemoteRefBehavior,
  remoteDiffOmissionNotice,
  remoteVersionActionLabel
} from '../remoteSkills.js';
import { LoadingNotice } from './common.jsx';

export function RemoteSourceBindingDialog({
  dialog,
  onBind,
  onBindCandidate,
  onClose,
  onSearch,
  onUpdate,
  onViewCandidate
}) {
  const hasCandidates = dialog.candidates.length > 0;

  return (
    <div
      className="modalBackdrop"
      role="presentation"
      onMouseDown={(event) => closeOnBackdropClick(event, onClose)}
    >
      <section className="remoteImportDialog" role="dialog" aria-modal="true" aria-labelledby="remote-source-title">
        <div className="importSheetHeader">
          <div>
            <h2 id="remote-source-title">Bind source</h2>
            <p>Link a GitHub source without replacing the current version.</p>
          </div>
          <button className="iconButton" disabled={dialog.loading} type="button" aria-label="Close source binding" onClick={onClose}>
            <X aria-hidden="true" />
          </button>
        </div>
        <form className="remoteImportForm" onSubmit={onBind}>
          <label className="remoteImportField">
            <span>GitHub source URL</span>
            <input
              autoFocus
              disabled={dialog.loading}
              placeholder="https://github.com/owner/repo/tree/main/path/to/skill"
              type="url"
              value={dialog.sourceUrl}
              onChange={(event) => onUpdate({ sourceUrl: event.target.value, preview: null })}
            />
          </label>
          <div className="remoteSourceCandidatePanel">
            <div className="remoteSourceCandidateHeader">
              <span>Suggested Claude Marketplace matches</span>
              <button className="inlineActionButton" disabled={dialog.searching || dialog.binding} type="button" onClick={onSearch}>
                <RefreshCw aria-hidden="true" size={14} />
                {dialog.searching ? 'Searching...' : 'Search again'}
              </button>
            </div>
            {dialog.searching ? (
              <LoadingNotice compact>
                Searching Claude Marketplace in the background. You can paste a GitHub URL or close this dialog while
                results load.
              </LoadingNotice>
            ) : hasCandidates ? (
              <div className="remoteSourceCandidateList">
                {dialog.candidates.map((candidate) => (
                  <div className="remoteSourceCandidateRow" key={candidate.sourceUrl}>
                    <span>
                      <strong>{candidate.repoLabel || candidate.repoUrl}</strong>
                      <small>{candidate.path}</small>
                      {candidate.description ? <small>{candidate.description}</small> : null}
                    </span>
                    <div className="remoteSourceCandidateMeta">
                      <small>score {candidate.score}</small>
                      <div className="remoteSourceCandidateActions">
                        <button
                          className="button secondary"
                          disabled={!candidate.sourceUrl}
                          type="button"
                          onClick={() => onViewCandidate(candidate)}
                        >
                          View
                        </button>
                        <button
                          className="button primary"
                          disabled={dialog.loading || dialog.binding || !candidate.sourceUrl}
                          type="button"
                          onClick={() => onBindCandidate(candidate)}
                        >
                          Bind
                        </button>
                      </div>
                    </div>
                    {candidate.matchReasons.length > 0 ? (
                      <div className="remoteSourceCandidateReasons">
                        {candidate.matchReasons.slice(0, 3).map((reason) => (
                          <small key={reason}>{reason}</small>
                        ))}
                      </div>
                    ) : null}
                  </div>
                ))}
              </div>
            ) : dialog.searched ? (
              <div className="remoteSourceCandidateNotice">
                {dialog.searchError || 'No Claude Marketplace candidates found. Paste a URL manually.'}
              </div>
            ) : null}
          </div>
          {dialog.preview ? (
            <div className="sourceBindingPreview">
              <strong>{dialog.preview.statusLabel}</strong>
              <span>{formatRemoteRefBehavior(dialog.preview)}</span>
              {dialog.preview.message ? <small>{dialog.preview.message}</small> : null}
            </div>
          ) : null}
          {dialog.error ? <div className="formError">{dialog.error}</div> : null}
          <div className="remoteImportFooter">
            <button className="button secondary" disabled={dialog.binding} type="button" onClick={onClose}>
              Cancel
            </button>
            <button className="button primary" disabled={dialog.loading || dialog.binding || !dialog.sourceUrl.trim()} type="submit">
              {dialog.loading ? (
                <>
                  <span className="buttonSpinner" aria-hidden="true" />
                  Verifying...
                </>
              ) : dialog.binding ? (
                <>
                  <span className="buttonSpinner" aria-hidden="true" />
                  Binding...
                </>
              ) : (
                'Verify and Bind Source'
              )}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}

export function RemoteSourceCandidateBindDialog({ dialog, skillName, onClose, onConfirm }) {
  const candidate = dialog.candidate || {};
  const canConfirm = dialog.preview && dialog.preview.validation !== 'mismatch' && !dialog.loading && !dialog.binding;

  return (
    <div
      className="modalBackdrop"
      role="presentation"
      onMouseDown={(event) => closeOnBackdropClick(event, onClose)}
    >
      <section
        className="remoteImportDialog remoteSourceConfirmDialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="remote-source-confirm-title"
      >
        <div className="importSheetHeader">
          <div>
            <h2 id="remote-source-confirm-title">Bind source</h2>
            <p>Confirm the GitHub source for {skillName} after validation passes.</p>
          </div>
          <button
            className="iconButton"
            disabled={dialog.binding}
            type="button"
            aria-label="Close source confirmation"
            onClick={onClose}
          >
            <X aria-hidden="true" />
          </button>
        </div>
        <div className="remoteImportForm">
          <div className="remoteSourceCandidateConfirmSummary">
            <strong>{candidate.repoLabel || candidate.repoUrl || 'Selected source'}</strong>
            {candidate.path ? <small>{candidate.path}</small> : null}
            {candidate.sourceUrl ? <small>{candidate.sourceUrl}</small> : null}
          </div>

          {dialog.loading ? (
            <LoadingNotice>Checking source...</LoadingNotice>
          ) : dialog.preview ? (
            <div className="sourceBindingPreview">
              <strong>{dialog.preview.statusLabel}</strong>
              <span>{formatRemoteRefBehavior(dialog.preview)}</span>
              {dialog.preview.message ? <small>{dialog.preview.message}</small> : null}
            </div>
          ) : null}

          {dialog.error ? <div className="formError">{dialog.error}</div> : null}

          <div className="remoteImportFooter">
            <button className="button secondary" disabled={dialog.binding} type="button" onClick={onClose}>
              Cancel
            </button>
            <button className="button primary" disabled={!canConfirm} type="button" onClick={onConfirm}>
              {dialog.binding ? (
                <>
                  <span className="buttonSpinner" aria-hidden="true" />
                  Binding...
                </>
              ) : (
                'Confirm bind'
              )}
            </button>
          </div>
        </div>
      </section>
    </div>
  );
}

export function RemoteVersionReviewDialog({ dialog, onActivatePath, onApply, onClose }) {
  const preview = dialog.preview;
  const activeFile =
    preview?.files.find((file) => file.path === dialog.activePath) ||
    preview?.files[0] ||
    null;
  const hasNoFileChanges = Boolean(preview && preview.files.length === 0);
  const allowNoFileChanges =
    hasNoFileChanges && Boolean(preview?.fromVersion && preview?.toVersion && preview.fromVersion !== preview.toVersion);
  const diffOmission = activeFile && !hasNoFileChanges ? remoteDiffOmissionNotice(activeFile) : null;
  const canApply = canApplyRemoteVersionChange({
    allowNoFileChanges,
    files: preview?.files || [],
    loading: dialog.loading || dialog.applying
  });

  return (
    <div
      className="modalBackdrop"
      role="presentation"
      onMouseDown={(event) => closeOnBackdropClick(event, onClose)}
    >
      <section className="syncDialog gitCommitDialog" role="dialog" aria-modal="true" aria-labelledby="remote-version-title">
        <div className="importSheetHeader">
          <div>
            <h2 id="remote-version-title">
              {preview ? `${remoteVersionActionLabel(preview)} ${preview.skillName}` : 'Review version change'}
            </h2>
            <p>{preview ? `${preview.fromVersion} -> ${preview.toVersion}` : 'Loading remote version diff.'}</p>
          </div>
          <button className="iconButton" disabled={dialog.applying} type="button" aria-label="Close version review" onClick={onClose}>
            <X aria-hidden="true" />
          </button>
        </div>
        <div className="gitCommitDialogBody">
          {dialog.loading ? <LoadingNotice>Loading diff...</LoadingNotice> : null}
          {preview ? (
            <div className="gitCommitReview">
              <aside className="gitFilePane">
                <div className="gitFilePaneHeader">
                  <strong>{preview.files.length} files</strong>
                </div>
                <div className="gitFileList">
                  {preview.files.length > 0 ? (
                    preview.files.map((file) => (
                      <button
                        className={activeFile?.path === file.path ? 'gitFileRow remoteFileRow active' : 'gitFileRow remoteFileRow'}
                        key={file.path}
                        type="button"
                        onClick={() => onActivatePath(file.path)}
                      >
                        <span>
                          <strong>{file.path}</strong>
                          <small>{file.label}</small>
                        </span>
                      </button>
                    ))
                  ) : (
                    <div className="gitEmptyState">No file changes.</div>
                  )}
                </div>
              </aside>
              <section className="gitDiffPane" aria-label="Remote version diff">
                <div className="gitDiffHeader">
                  <strong>{activeFile?.path || 'Diff'}</strong>
                  {activeFile ? <span>{activeFile.label}</span> : null}
                </div>
                {hasNoFileChanges ? (
                  <div className="gitDiffEmpty noFileChanges">
                    <strong>No file changes in this skill</strong>
                    <span>Applying records the latest source revision without changing local files.</span>
                  </div>
                ) : diffOmission ? (
                  <div className="gitDiffEmpty diffOmission">
                    <strong>{diffOmission.title}</strong>
                    <span>{diffOmission.detail}</span>
                    <code>{diffOmission.sizeSummary}</code>
                    <code>{diffOmission.hashSummary}</code>
                  </div>
                ) : (
                  <GitDiffView diff={activeFile?.diff || ''} />
                )}
              </section>
            </div>
          ) : null}
          {dialog.error ? <div className="formError remoteDialogError">{dialog.error}</div> : null}
        </div>
        <div className="remoteImportFooter remoteDialogFooter">
          <button className="button secondary" disabled={dialog.applying} type="button" onClick={onClose}>
            Cancel
          </button>
          <button className="button primary" disabled={!canApply} type="button" onClick={onApply}>
            {dialog.applying ? (
              <>
                <span className="buttonSpinner" aria-hidden="true" />
                Applying...
              </>
            ) : (
              'Apply change'
            )}
          </button>
        </div>
      </section>
    </div>
  );
}
