import React from 'react';
import { RefreshCw } from 'lucide-react';
import { GitDiffView } from '../GitDiffView.jsx';
import { closeOnBackdropClick } from '../modalEvents.js';
import { canCommitUserSkillsChanges } from '../userSkillsGitSync.js';

export function UserSkillsSyncDialog({
  dialog,
  status,
  onActivatePath,
  onClose,
  onGenerateMessage,
  onOpenSettings,
  onSelectAllPaths,
  onSubmit,
  onTogglePath,
  onUpdate
}) {
  const selected = new Set(dialog.selectedPaths);
  const activeFile =
    dialog.changes.files.find((file) => file.path === dialog.activePath) ||
    dialog.changes.files[0] ||
    null;
  const allSelected =
    dialog.changes.files.length > 0 && dialog.selectedPaths.length === dialog.changes.files.length;
  const isBusy = status === 'syncing' || dialog.loading;
  const canSubmit = canCommitUserSkillsChanges({
    files: dialog.changes.files,
    loading: dialog.loading,
    push: dialog.push,
    remoteUrl: dialog.remoteUrl,
    selectedPaths: dialog.selectedPaths,
    status
  });
  const submitLabel =
    status === 'syncing'
      ? 'Committing...'
      : dialog.changes.files.length === 0
        ? 'No changes'
        : 'Commit and sync';

  return (
    <div
      className="modalBackdrop"
      role="presentation"
      onMouseDown={(event) => closeOnBackdropClick(event, onClose)}
    >
      <section className="syncDialog gitCommitDialog" role="dialog" aria-modal="true" aria-labelledby="user-skills-sync-title">
        <div className="importSheetHeader">
          <div>
            <h2 id="user-skills-sync-title">Review user skills commit</h2>
            <p>Choose the files to commit, review the diff, then sync the shared user skills repo.</p>
          </div>
          <button className="iconButton" disabled={isBusy} type="button" aria-label="Close user skills commit review" onClick={onClose}>
            x
          </button>
        </div>

        <form className="gitCommitForm" onSubmit={onSubmit}>
          <div className="gitCommitFields">
            <label className="remoteImportField">
              <span className="fieldLabelRow">
                <span>Commit message</span>
                <button className="inlineActionButton" disabled={isBusy} type="button" onClick={onGenerateMessage}>
                  <RefreshCw aria-hidden="true" size={14} />
                  Generate
                </button>
              </span>
              <input
                autoFocus
                disabled={isBusy}
                name="commit-message"
                value={dialog.commitMessage}
                onChange={(event) => onUpdate({ commitMessage: event.target.value })}
              />
            </label>
            <label className="remoteImportField">
              <span className="fieldLabelRow">
                <span>Remote URL</span>
                <button className="inlineActionButton" disabled={isBusy} type="button" onClick={onOpenSettings}>
                  Edit in Settings
                </button>
              </span>
              <input
                name="remote-url"
                placeholder="git@github.com:santosli/user-skills.git"
                readOnly
                value={dialog.remoteUrl || 'Not configured'}
              />
            </label>
          </div>

          <label className="syncCheckbox">
            <input
              checked={dialog.push}
              disabled={isBusy}
              name="push-after-commit"
              type="checkbox"
              onChange={(event) => onUpdate({ push: event.target.checked })}
            />
            <span>Push after commit</span>
          </label>

          {dialog.syncLog.length > 0 ? (
            <div className="syncProgressPanel" aria-live="polite">
              <span className="syncSpinner" aria-hidden="true" />
              <div>
                <strong>{dialog.push ? 'Committing and pushing' : 'Committing locally'}</strong>
                <ol>
                  {dialog.syncLog.map((line, index) => (
                    <li key={line} className={index === 0 ? 'active' : ''}>
                      {line}
                    </li>
                  ))}
                </ol>
              </div>
            </div>
          ) : null}

          <div className="gitCommitReview">
            <aside className="gitFilePane">
              <div className="gitFilePaneHeader">
                <strong>{dialog.selectedPaths.length} selected</strong>
                <label className="syncCheckbox compact">
                  <input
                    checked={allSelected}
                    disabled={isBusy || dialog.changes.files.length === 0}
                    name="select-all-files"
                    type="checkbox"
                    onChange={(event) => onSelectAllPaths(event.target.checked)}
                  />
                  <span>All</span>
                </label>
              </div>

              {dialog.loading ? (
                <div className="gitEmptyState">Loading changes...</div>
              ) : dialog.changes.files.length === 0 ? (
                <div className="gitEmptyState">No changed files.</div>
              ) : (
                <div className="gitFileList">
                  {dialog.changes.files.map((file) => (
                    <label
                      className={activeFile?.path === file.path ? 'gitFileRow active' : 'gitFileRow'}
                      key={file.path}
                      onClick={() => onActivatePath(file.path)}
                    >
                      <input
                        checked={selected.has(file.path)}
                        disabled={isBusy}
                        name="selected-files"
                        type="checkbox"
                        onChange={(event) => onTogglePath(file.path, event.target.checked)}
                      />
                      <span>
                        <strong>{file.path}</strong>
                        <small>{file.label}</small>
                      </span>
                    </label>
                  ))}
                </div>
              )}
            </aside>

            <section className="gitDiffPane" aria-label="Selected file diff">
              <div className="gitDiffHeader">
                <strong>{activeFile?.path || 'Diff'}</strong>
                {activeFile ? <span>{activeFile.label}</span> : null}
              </div>
              <GitDiffView diff={activeFile?.diff || ''} />
            </section>
          </div>

          {dialog.error ? <div className="formError">{dialog.error}</div> : null}

          <div className="remoteImportFooter">
            <button className="button secondary" disabled={isBusy} type="button" onClick={onClose}>
              Cancel
            </button>
            <button className="button primary" disabled={!canSubmit} type="submit">
              {submitLabel}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}
