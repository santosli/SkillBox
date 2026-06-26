import React, { useEffect, useState } from 'react';
import {
  ExternalLink,
  FolderOpen,
  Link2,
  Star,
  X
} from 'lucide-react';
import { normalizeEditableTags } from '../dashboardMetadata.js';
import { closeOnBackdropClick } from '../modalEvents.js';
import {
  formatOperationTimestamp,
  remoteSkillUpdateVersionLabel,
  shouldShowRemoteUpdateSummary
} from '../remoteSkills.js';
import { labelize } from '../skills.js';
import { userSyncAction } from '../userSkillsGitSync.js';
import { AgentIconStack, Badge, LoadingNotice } from './common.jsx';

function RemoteSkillControlPanel({
  isChecking,
  loading,
  remoteUpdate,
  onBindRemoteSource,
  onCheckUpdates,
  onReviewUpdate
}) {
  const sourceMissing = remoteUpdate?.state === 'no_source';
  const sourceLinked = Boolean(remoteUpdate && remoteUpdate.state !== 'no_source');
  const sourceLabel = sourceMissing
    ? 'No source configured'
    : remoteUpdate?.state === 'pinned'
      ? 'Pinned source'
      : remoteUpdate
        ? 'GitHub source linked'
        : 'Source not checked';
  const updateLabel = remoteUpdate?.stateLabel || 'Update not checked';
  const showUpdateSummary = remoteUpdate?.state !== 'no_source' && shouldShowRemoteUpdateSummary(remoteUpdate);
  const showReviewUpdate = remoteUpdate?.updateAvailable === true;
  const updateSectionLabel = showReviewUpdate ? 'Ready to review' : updateLabel;
  const updateSummaryTitle = remoteUpdate?.state === 'pinned'
    ? 'Pinned source'
    : showReviewUpdate
      ? 'Version change'
      : remoteUpdate?.stateLabel || remoteUpdate?.state;
  const updateMessage =
    showReviewUpdate && /update available/i.test(remoteUpdate?.message || '') ? '' : remoteUpdate?.message || '';

  return (
    <section className="remoteSkillPanel" aria-label="Remote skill controls">
      <div className="skillDetailControlSection">
        <div className="skillDetailSectionHeader">
          <span>Remote source</span>
          <small>{sourceLabel}</small>
        </div>
        <p className="skillDetailControlCopy">
          {sourceMissing || !remoteUpdate
            ? 'Bind a source before checking or applying remote updates.'
            : 'Source changes are linked without replacing the current version.'}
        </p>
        <button
          className={sourceMissing || !remoteUpdate ? 'button primary' : 'button secondary'}
          type="button"
          onClick={onBindRemoteSource}
        >
          {sourceLinked ? 'Rebind source' : 'Bind source'}
        </button>
      </div>

      {!sourceMissing ? <div className="skillDetailControlSection">
        <div className="skillDetailSectionHeader">
          <span>Updates</span>
          <small>{updateSectionLabel}</small>
        </div>
        {loading ? <LoadingNotice compact>Loading remote details...</LoadingNotice> : null}
        {showUpdateSummary ? (
          <div className="remoteVersionSummary">
            <strong>{updateSummaryTitle}</strong>
            <span>{remoteSkillUpdateVersionLabel(remoteUpdate)}</span>
            {updateMessage ? <small>{updateMessage}</small> : null}
          </div>
        ) : null}
        <div className="skillDetailControlActions">
          {showReviewUpdate ? (
            <button
              className="button primary"
              type="button"
              onClick={onReviewUpdate}
            >
              Review update
            </button>
          ) : null}
          <button className="button secondary" disabled={isChecking} type="button" onClick={() => onCheckUpdates()}>
            {isChecking ? (
              <>
                <span className="buttonSpinner" aria-hidden="true" />
                Checking...
              </>
            ) : (
              'Check update'
            )}
          </button>
        </div>
      </div> : null}
    </section>
  );
}

function UserSkillControlPanel({ isPreparingSync, isSyncing, syncAction, onOpenSyncSetup }) {
  return (
    <section className="userSkillPanel" aria-label="User skill controls">
      <div className="skillDetailControlSection">
        <div className="skillDetailSectionHeader">
          <span>User sync</span>
          <small>{isSyncing ? 'Sync in progress' : 'Local skill'}</small>
        </div>
        <p className="skillDetailControlCopy">
          Commit and push user skill changes from the managed SkillBox store.
        </p>
        <button
          className="button primary"
          disabled={isSyncing}
          type="button"
          onClick={onOpenSyncSetup}
        >
          {isPreparingSync ? 'Preparing...' : isSyncing ? 'Syncing...' : syncAction}
        </button>
      </div>
    </section>
  );
}

function UserSkillVersionHistoryPanel({ loading, versions }) {
  const versionCount = versions?.versions?.length || 0;

  return (
    <section className="skillDetailVersionHistory" aria-label="User skill version history">
      <div className="skillDetailSectionHeader">
        <span>Version history</span>
        <small>{versionCount ? `${versionCount} versions` : loading ? 'Loading' : 'No versions loaded'}</small>
      </div>
      {loading ? <LoadingNotice compact>Loading local details...</LoadingNotice> : null}
      {versionCount ? (
        <RemoteVersionsPanel versions={versions} ariaLabel="User skill versions" />
      ) : !loading ? (
        <div className="skillDetailEmptyPanel">No version history loaded.</div>
      ) : null}
    </section>
  );
}

const VERSION_HISTORY_PREVIEW_COUNT = 3;

function RemoteVersionHistoryPanel({ loading, versions, onReviewRollback }) {
  const versionCount = versions?.versions?.length || 0;

  return (
    <section className="skillDetailVersionHistory" aria-label="Version history">
      <div className="skillDetailSectionHeader">
        <span>Version history</span>
        <small>{versionCount ? `${versionCount} versions` : loading ? 'Loading' : 'No versions loaded'}</small>
      </div>
      {loading ? <LoadingNotice compact>Loading remote details...</LoadingNotice> : null}
      {versionCount ? (
        <RemoteVersionsPanel
          versions={versions}
          ariaLabel="Remote skill versions"
          onReviewRollback={onReviewRollback}
        />
      ) : !loading ? (
        <div className="skillDetailEmptyPanel">No version history loaded.</div>
      ) : null}
    </section>
  );
}

function RemoteVersionsPanel({ ariaLabel = 'Skill versions', versions, onReviewRollback }) {
  const [expanded, setExpanded] = useState(false);
  const versionRows = versions?.versions || [];
  const versionResetKey = versionRows.map((version) => version.version).join('|');

  useEffect(() => {
    setExpanded(false);
  }, [versions?.skillName, versionResetKey]);

  if (!versionRows.length) {
    return null;
  }

  const hiddenVersionCount = Math.max(0, versionRows.length - VERSION_HISTORY_PREVIEW_COUNT);
  const hasHiddenVersions = hiddenVersionCount > 0;
  const visibleVersions = expanded || !hasHiddenVersions
    ? versionRows
    : versionRows.slice(0, VERSION_HISTORY_PREVIEW_COUNT);

  return (
    <div className="remoteVersionList" aria-label={ariaLabel}>
      {visibleVersions.map((version) => {
        const versionMeta = [
          version.isCurrent ? 'Current' : version.kind,
          version.message,
          version.updatedAt ? `Updated ${formatOperationTimestamp(version.updatedAt)}` : ''
        ].filter(Boolean).join(' · ');

        return (
          <div
            className={`remoteVersionRow${version.isCurrent ? ' current' : ''}`}
            aria-current={version.isCurrent ? 'true' : undefined}
            key={version.version}
          >
            <span>
              <strong>{version.shortLabel || version.version}</strong>
              <small>{versionMeta}</small>
            </span>
            {version.isCurrent ? (
              <span className="button secondary remoteVersionCurrentBadge">Active</span>
            ) : onReviewRollback ? (
              <button
                className="button secondary"
                type="button"
                onClick={() => onReviewRollback(version)}
              >
                Rollback
              </button>
            ) : null}
          </div>
        );
      })}
      {hasHiddenVersions ? (
        <button
          className="remoteVersionToggle"
          type="button"
          onClick={() => setExpanded((current) => !current)}
        >
          {expanded ? 'Show fewer' : `Show ${hiddenVersionCount} more`}
        </button>
      ) : null}
    </div>
  );
}

function OperationHistoryPanel({ operations }) {
  if (!operations?.length) {
    return null;
  }

  return (
    <details className="operationHistoryPanel" aria-label="Operation history">
      <summary className="operationHistorySummary">
        <span>Operation log</span>
        <small>{operations.length} events</small>
      </summary>
      <div className="operationHistoryRows">
        {operations.slice(0, 4).map((operation) => {
          const operationTimestamp = formatOperationTimestamp(operation.finishedAt || operation.startedAt);

          return (
            <div className="operationHistoryRow" key={operation.id}>
              <span>{operation.summary || operation.operationType}</span>
              {operationTimestamp ? (
                <time dateTime={operation.finishedAt || operation.startedAt}>{operationTimestamp}</time>
              ) : null}
              <Badge tone={operation.status === 'failed' ? 'red' : 'slate'}>{operation.status}</Badge>
            </div>
          );
        })}
      </div>
    </details>
  );
}

export function SkillDetailDialog({
  skill,
  operations,
  remoteLoading,
  remoteUpdate,
  status,
  userLoading,
  userSkillsGit,
  userVersions,
  versions,
  onBindRemoteSource,
  onCheckUpdates,
  onClose,
  onOpenDeployDialog,
  onOpenLocalFolder,
  onOpenSourceUrl,
  onOpenSyncSetup,
  onReviewRollback,
  onReviewUpdate,
  sourceUrl,
  onTagsChange,
  onToggleFavorite
}) {
  const [tagInput, setTagInput] = useState('');
  const syncAction = userSyncAction(userSkillsGit, skill.type);
  const isPreparingSync = status === 'preparing_sync';
  const isSyncing = status === 'syncing' || isPreparingSync;
  const isChecking = status === 'checking';
  const pendingTag = normalizeEditableTags([tagInput])[0] || '';

  useEffect(() => {
    function closeOnEscape(event) {
      if (event.key === 'Escape') {
        onClose();
      }
    }

    window.addEventListener('keydown', closeOnEscape);
    return () => window.removeEventListener('keydown', closeOnEscape);
  }, [onClose]);

  function addTag(event) {
    event.preventDefault();
    if (!pendingTag) {
      return;
    }

    onTagsChange(skill.name, [...skill.displayTags, pendingTag]);
    setTagInput('');
  }

  function removeTag(tag) {
    onTagsChange(
      skill.name,
      skill.displayTags.filter((item) => item !== tag)
    );
  }

  return (
    <div
      className="modalBackdrop skillDetailBackdrop"
      role="presentation"
      onMouseDown={(event) => closeOnBackdropClick(event, onClose)}
    >
      <section
        className="skillDetailDialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="skill-detail-title"
      >
        <header className="skillDetailDialogHeader">
          <div className="skillDetailTitleBlock">
            <div className="skillDetailBadges">
              <Badge tone={skill.type === 'user' ? 'green' : 'blue'}>{labelize(skill.type)}</Badge>
              <Badge tone={skill.statusTone}>{skill.statusLabel}</Badge>
            </div>
            <div className="skillDetailTitleRow">
              <h2 id="skill-detail-title">{skill.name}</h2>
              {skill.path ? (
                <button
                  aria-label={`Open ${skill.name} local folder`}
                  className="button secondary skillDetailSourceButton"
                  type="button"
                  onClick={() => onOpenLocalFolder(skill)}
                >
                  <FolderOpen aria-hidden="true" />
                  Folder
                </button>
              ) : null}
              {sourceUrl ? (
                <button
                  aria-label={`Open ${skill.name} source`}
                  className="button secondary skillDetailSourceButton"
                  type="button"
                  onClick={() => onOpenSourceUrl(sourceUrl)}
                >
                  <ExternalLink aria-hidden="true" />
                  Source
                </button>
              ) : null}
            </div>
            <p className="skillDetailDescription">
              {skill.description || 'No description in SKILL.md frontmatter.'}
            </p>
          </div>
          <div className="skillDetailHeaderActions">
            <button
              aria-pressed={skill.isFavorite}
              className={skill.isFavorite ? 'detailFavoriteButton active' : 'detailFavoriteButton'}
              type="button"
              onClick={() => onToggleFavorite(skill.name)}
            >
              <Star aria-hidden="true" />
              {skill.isFavorite ? 'Favorited' : 'Favorite'}
            </button>
            <button className="iconButton skillDetailCloseButton" type="button" aria-label="Close skill detail" onClick={onClose}>
              <X aria-hidden="true" />
            </button>
          </div>
        </header>

        <div className="skillDetailBodyGrid">
          <div className="skillDetailMetaColumn">
            <section className="skillDetailSection skillDetailDeploySection" aria-label="Deploy workspace">
              <div className="skillDetailSectionHeader">
                <span>Workspace deployment</span>
                <button className="button secondary compactAction" type="button" onClick={onOpenDeployDialog}>
                  <Link2 aria-hidden="true" />
                  Deploy
                </button>
              </div>
              <div className="skillDetailDeploySurface">
                <div className="skillDetailDeployMetrics">
                  <div className="skillDetailDeploySummary">
                    <span className="skillDetailDeployMetric">{skill.installedAgents.length || 0}</span>
                    <div>
                      <div className="skillDetailDeployLabelRow">
                        <strong>Active workspaces</strong>
                        <AgentIconStack
                          agents={skill.installedAgents}
                          emptyLabel="No deployed workspace"
                          labelPrefix="Deploy workspaces"
                        />
                      </div>
                      <small>{skill.installedAgents.length ? 'Active runtime workspaces' : 'No workspace deployed'}</small>
                    </div>
                  </div>
                  <div className="skillDetailUsageSummary">
                    <span className="skillDetailDeployMetric">{skill.usageCount || 0}</span>
                    <div>
                      <strong>Usage</strong>
                      <small>Agent calls recorded</small>
                    </div>
                  </div>
                </div>
              </div>
            </section>

            {skill.type === 'remote' ? (
              <>
                <RemoteVersionHistoryPanel
                  loading={remoteLoading}
                  versions={versions}
                  onReviewRollback={onReviewRollback}
                />
                <OperationHistoryPanel operations={operations} />
              </>
            ) : skill.type === 'user' ? (
              <UserSkillVersionHistoryPanel
                loading={userLoading}
                versions={userVersions}
              />
            ) : null}
          </div>

          <aside className="skillDetailControlRail" aria-label="Skill controls">
            <div className="skillDetailRailHeader">
              <span>Controls</span>
              <small>{isChecking ? 'Checking remote' : isSyncing ? 'Working' : 'Ready'}</small>
            </div>
            <section className="skillDetailControlSection skillDetailTagsControl" aria-label="Skill tags">
              <div className="skillDetailSectionHeader">
                <span>Tags</span>
                <small>{skill.displayTags.length} labels</small>
              </div>
              <form className="skillDetailTagEditor" onSubmit={addTag}>
                <div className="skillDetailTagList" aria-label="Skill tags">
                  {skill.displayTags.map((tag) => (
                    <button
                      aria-label={`Remove ${tag} tag`}
                      className="editableTagPill"
                      key={tag}
                      type="button"
                      onClick={() => removeTag(tag)}
                    >
                      <span>{tag}</span>
                      <X aria-hidden="true" />
                    </button>
                  ))}
                </div>
                <div className="skillDetailTagInput">
                  <input
                    aria-label="Add tag"
                    name="skill-detail-tag"
                    placeholder="new tag"
                    value={tagInput}
                    onChange={(event) => setTagInput(event.target.value)}
                  />
                  <button disabled={!pendingTag} type="submit">
                    Add
                  </button>
                </div>
              </form>
            </section>
            {skill.type === 'remote' ? (
              <RemoteSkillControlPanel
                isChecking={isChecking}
                loading={remoteLoading}
                remoteUpdate={remoteUpdate}
                onBindRemoteSource={onBindRemoteSource}
                onCheckUpdates={onCheckUpdates}
                onReviewUpdate={onReviewUpdate}
              />
            ) : (
              <UserSkillControlPanel
                isPreparingSync={isPreparingSync}
                isSyncing={isSyncing}
                syncAction={syncAction}
                onOpenSyncSetup={onOpenSyncSetup}
              />
            )}
          </aside>
        </div>

      </section>
    </div>
  );
}
