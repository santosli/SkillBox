import React, { useEffect, useRef } from 'react';
import { Copy, FolderOpen, RefreshCw, ShieldAlert, X } from 'lucide-react';
import { GitDiffView } from '../GitDiffView.jsx';
import {
  canApplyUserSkillsInbound,
  beginReviewDialogFocus,
  handleReviewDialogKeyDown,
  inboundConflictDiagnosticGroups,
  inboundFileLabel,
  inboundRelationLabel
} from '../userSkillsInbound.js';
import { closeOnBackdropClick } from '../modalEvents.js';

function changeLabel(change) {
  if (change.kind === 'renamed') {
    return `${change.previousName} → ${change.skillName}`;
  }
  return change.skillName;
}

export function UserSkillsInboundReviewDialog({
  dialog,
  onActivatePath,
  onApply,
  onClose,
  onCopyRepositoryPath,
  onOpenRepository,
  onRefresh,
  restoreFocusFallback
}) {
  const dialogRef = useRef(null);
  const closeButtonRef = useRef(null);
  const restoreFocusRef = useRef(
    typeof document === 'undefined' ? null : document.activeElement
  );
  const preview = dialog.preview;
  const activeFile =
    preview?.files.find((file) => file.path === dialog.activePath) ||
    preview?.files[0] ||
    null;
  const busy = Boolean(dialog.loading || dialog.applying);
  const canApply = canApplyUserSkillsInbound(preview, busy);
  const isDiverged = preview?.status.relation === 'diverged';
  const isDirty = preview?.status.worktreeState === 'dirty';
  const isSafeBootstrap =
    preview?.status.relation === 'remote_only' && isDirty && Boolean(preview?.canApply);
  const isDirtyBlocking = isDirty && !preview?.canApply;

  useEffect(
    () =>
      beginReviewDialogFocus(
        closeButtonRef.current,
        restoreFocusRef.current,
        restoreFocusFallback
      ),
    []
  );

  return (
    <div
      className="modalBackdrop"
      role="presentation"
      onMouseDown={(event) => closeOnBackdropClick(event, onClose)}
    >
      <section
        aria-labelledby="user-skills-inbound-title"
        aria-modal="true"
        className="syncDialog gitCommitDialog inboundReviewDialog"
        ref={dialogRef}
        role="dialog"
        onKeyDown={(event) =>
          handleReviewDialogKeyDown(event, {
            dialogElement: dialogRef.current,
            onClose,
            closeDisabled: dialog.applying
          })
        }
      >
        <div className="importSheetHeader">
          <div>
            <h2 id="user-skills-inbound-title">Review incoming changes</h2>
            <p>
              Review the repository-wide update before SkillBox fast-forwards{' '}
              <code>origin/main</code>.
            </p>
          </div>
          <button
            aria-label="Close incoming changes review"
            className="iconButton"
            disabled={dialog.applying}
            ref={closeButtonRef}
            type="button"
            onClick={onClose}
          >
            <X aria-hidden="true" />
          </button>
        </div>

        <div className="gitCommitDialogBody inboundReviewBody">
          {dialog.loading ? <div className="loadingNotice">Checking remote repository...</div> : null}
          {preview ? (
            <>
              <div className="inboundReviewSummary">
                <div>
                  <span>Relation</span>
                  <strong>{inboundRelationLabel(preview.status)}</strong>
                </div>
                <div>
                  <span>Incoming commits</span>
                  <strong>{preview.status.behindCount}</strong>
                </div>
                <div>
                  <span>Worktree</span>
                  <strong>{isSafeBootstrap ? 'Bootstrap safe' : isDirty ? 'Dirty' : 'Clean'}</strong>
                </div>
                <div>
                  <span>Branch</span>
                  <strong>{preview.status.branch}</strong>
                </div>
              </div>

              {isSafeBootstrap ? (
                <div className="inboundBootstrapNotice">
                  <strong>Remote repository setup is ready</strong>
                  <span>
                    The local repository contains only SkillBox setup content. Applying initializes
                    it from <code>origin/main</code> without overwriting user skill files.
                  </span>
                </div>
              ) : null}

              {isDirtyBlocking ? (
                <div className="inboundBlockingNotice">
                  <ShieldAlert aria-hidden="true" />
                  <div>
                    <strong>Local changes must be handled first</strong>
                    <span>
                      SkillBox will not stash or overwrite a dirty worktree. Commit or discard changes
                      with normal Git tooling, then Refresh.
                    </span>
                  </div>
                </div>
              ) : null}

              {isDiverged ? (
                <ConflictDiagnosis analysis={preview.conflictAnalysis} />
              ) : (
                <>
                  <InboundChangeSummary preview={preview} />
                  <div className="gitCommitReview inboundDiffReview">
                    <aside className="gitFilePane">
                      <div className="gitFilePaneHeader">
                        <strong>{preview.files.length} changed files</strong>
                      </div>
                      <div className="gitFileList">
                        {preview.files.length ? (
                          preview.files.map((file) => (
                            <button
                              aria-current={activeFile?.path === file.path ? 'true' : undefined}
                              aria-pressed={activeFile?.path === file.path}
                              className={
                                activeFile?.path === file.path
                                  ? 'gitFileRow remoteFileRow active'
                                  : 'gitFileRow remoteFileRow'
                              }
                              key={file.path}
                              type="button"
                              onClick={() => onActivatePath(file.path)}
                            >
                              <span>
                                <strong>{file.path}</strong>
                                <small>{inboundFileLabel(file)}</small>
                              </span>
                            </button>
                          ))
                        ) : (
                          <div className="gitEmptyState">No file changes.</div>
                        )}
                      </div>
                    </aside>
                    <section aria-label="Incoming file diff" className="gitDiffPane">
                      <div className="gitDiffHeader">
                        <strong>{activeFile?.path || 'Diff'}</strong>
                        {activeFile ? <span>{inboundFileLabel(activeFile)}</span> : null}
                      </div>
                      <GitDiffView diff={activeFile?.diff || ''} />
                    </section>
                  </div>
                </>
              )}

              {preview.safetyIssues.length ? (
                <div className="inboundSafetyIssues" aria-label="Safety review">
                  <strong>Safety review</strong>
                  {preview.safetyIssues.map((issue) => (
                    <div className={issue.blocking ? 'blocking' : ''} key={`${issue.code}:${issue.path}`}>
                      <span>{issue.message}</span>
                      {issue.path ? <code>{issue.path}</code> : null}
                    </div>
                  ))}
                </div>
              ) : null}

              {preview.blockedReason ? (
                <div className="formError remoteDialogError">{preview.blockedReason}</div>
              ) : null}
            </>
          ) : null}
          {dialog.error ? <div className="formError remoteDialogError">{dialog.error}</div> : null}
        </div>

        <div className="remoteImportFooter remoteDialogFooter inboundReviewFooter">
          <div className="inboundRepositoryActions">
            <button className="button secondary" disabled={busy} type="button" onClick={onOpenRepository}>
              <FolderOpen aria-hidden="true" />
              Open repository
            </button>
            <button
              className="iconButton"
              aria-label="Copy repository path"
              disabled={busy}
              title="Copy repository path"
              type="button"
              onClick={onCopyRepositoryPath}
            >
              <Copy aria-hidden="true" />
            </button>
            <button className="button secondary" disabled={busy} type="button" onClick={onRefresh}>
              <RefreshCw aria-hidden="true" />
              Refresh
            </button>
          </div>
          <div className="appUpdateActions">
            <button className="button secondary" disabled={dialog.applying} type="button" onClick={onClose}>
              Cancel
            </button>
            <button className="button primary" disabled={!canApply} type="button" onClick={onApply}>
              {dialog.applying ? 'Applying...' : 'Apply fast-forward'}
            </button>
          </div>
        </div>
      </section>
    </div>
  );
}

function InboundChangeSummary({ preview }) {
  return (
    <div className="inboundChangeSummary">
      <section>
        <h3>Skill changes</h3>
        {preview.skillChanges.length ? (
          <div className="inboundChangeChips">
            {preview.skillChanges.map((change) => (
              <span className={`inboundChangeChip ${change.kind}`} key={`${change.kind}:${changeLabel(change)}`}>
                <strong>{change.kind}</strong>
                {changeLabel(change)}
              </span>
            ))}
          </div>
        ) : (
          <small>No skill directory changes.</small>
        )}
      </section>
      <section>
        <h3>Affected runtime targets</h3>
        {preview.skillChanges.some((change) => change.affectedDeployments.length) ? (
          <div className="inboundAffectedTargets">
            {preview.skillChanges.flatMap((change) =>
              change.affectedDeployments.map((deployment) => (
                <span key={`${change.skillName}:${deployment.targetRoot}`}>
                  <strong>{change.skillName}</strong>
                  {deployment.profileName || deployment.profileId || deployment.mode || 'Runtime target'}
                  <code>{deployment.targetPath || deployment.targetRoot}</code>
                </span>
              ))
            )}
          </div>
        ) : (
          <small>No deployed runtime targets are affected.</small>
        )}
      </section>
    </div>
  );
}

function ConflictDiagnosis({ analysis }) {
  const groups = inboundConflictDiagnosticGroups(analysis);

  return (
    <div className="inboundConflictDiagnosis">
      <div className="inboundConflictHeader">
        <ShieldAlert aria-hidden="true" />
        <span>
          <strong>Local and remote histories have diverged</strong>
          SkillBox will not merge, rebase, reset, or choose a side. Resolve the repository with
          normal Git tooling outside SkillBox, then Refresh.
        </span>
      </div>
      <dl>
        <div>
          <dt>Local-only commits</dt>
          <dd>{analysis?.localOnlyCommits || 0}</dd>
        </div>
        <div>
          <dt>Remote-only commits</dt>
          <dd>{analysis?.remoteOnlyCommits || 0}</dd>
        </div>
        <div>
          <dt>Skills changed by both</dt>
          <dd>{analysis?.bothChangedSkills.length || 0}</dd>
        </div>
        <div>
          <dt>Files changed by both</dt>
          <dd>{analysis?.bothChangedFiles.length || 0}</dd>
        </div>
        <div>
          <dt>Likely conflict files</dt>
          <dd>{analysis?.likelyConflictFiles.length || 0}</dd>
        </div>
      </dl>
      <div className="inboundConflictLists">
        {groups.map(({ id, label, items }) =>
          items.length ? (
            <details key={id}>
              <summary>
                {label} ({items.length})
              </summary>
              <ul>
                {items.slice(0, 8).map((item) => (
                  <li key={item}>
                    <code>{item}</code>
                  </li>
                ))}
              </ul>
              {items.length > 8 ? <small>Showing 8 of {items.length} items.</small> : null}
            </details>
          ) : null
        )}
      </div>
    </div>
  );
}
