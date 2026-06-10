import React from 'react';
import {
  AlertTriangle,
  Check,
  Link2,
  Plus,
  RefreshCw,
  Trash2,
  Unlink,
  X
} from 'lucide-react';
import { closeOnBackdropClick } from '../modalEvents.js';
import {
  workspaceCardMetaLabels,
  workspaceDeployChangeCount,
  workspaceDeploymentChanges,
  workspaceDeployRequiresConfirmation
} from '../workspaces.js';
import { AgentIconBadge, Badge } from './common.jsx';
import { DashboardStatusNotice } from './dashboard.jsx';

export function WorkspacePage({
  error,
  filter,
  notice,
  status,
  tabs,
  workspaces,
  onAdd,
  onDismissNotice,
  onFilter,
  onForget,
  onOpenSkills,
  onScan
}) {
  const isScanning = status === 'scanning_workspaces';
  const isOpeningWorkspace = status === 'scanning_workspace_skills';

  return (
    <section className="dashboardFrame workspaceFrame" aria-label="Workspace registry">
      {error ? <div className="notice">{error}</div> : null}
      <div className="dashboardTitleRow">
        <div className="dashboardTitleGroup">
          <h1>Workspaces</h1>
          <span className="dashboardCountPill">{workspaces.length}</span>
        </div>
      </div>

      <div className="dashboardControlRow workspaceControlRow">
        <div className="dashboardTypeTabs workspaceTypeTabs" role="tablist" aria-label="Workspace type">
          {tabs.map((tab) => (
            <button
              aria-selected={filter === tab.id}
              className={filter === tab.id ? 'active' : ''}
              key={tab.id}
              role="tab"
              type="button"
              onClick={() => onFilter(tab.id)}
            >
              <span>{tab.label}</span>
              <small>{tab.count}</small>
            </button>
          ))}
        </div>
        <div className="workspaceHeaderActions">
          <button className="button secondary" disabled={isScanning} type="button" onClick={onScan}>
            <RefreshCw aria-hidden="true" />
            {isScanning ? 'Scanning...' : 'Scan'}
          </button>
          <button className="button primary" disabled={isScanning} type="button" onClick={onAdd}>
            <Plus aria-hidden="true" />
            Add workspace
          </button>
        </div>
      </div>

      {notice ? (
        <DashboardStatusNotice message={notice} onDismiss={onDismissNotice} />
      ) : null}

      {workspaces.length > 0 ? (
        <div className="workspaceCardGrid" aria-label="Workspace cards">
          {workspaces.map((workspace) => (
            <WorkspaceCard
              isBusy={isScanning || isOpeningWorkspace}
              key={workspace.canonicalPath}
              workspace={workspace}
              onForget={onForget}
              onOpenSkills={onOpenSkills}
            />
          ))}
        </div>
      ) : (
        <div className="emptyState dashboardEmptyState workspaceEmptyState">
          <strong>No workspaces found</strong>
          <span>Run Scan or add an existing skills root.</span>
        </div>
      )}
    </section>
  );
}

function WorkspaceCard({ isBusy, workspace, onForget, onOpenSkills }) {
  const metaValues = {
    Scope: <Badge tone={workspace.kind === 'global' ? 'blue' : 'green'}>{workspace.kindLabel}</Badge>,
    Skills: <strong>{workspace.skillCount}</strong>,
    Imported: <strong>{workspace.importedSkillCount}</strong>,
    Calls: <strong>{workspace.usageCount}</strong>
  };

  return (
    <article className={workspace.kind === 'global' ? 'workspaceCard global' : 'workspaceCard'}>
      <button
        className="workspaceCardOpenButton"
        disabled={isBusy}
        type="button"
        onClick={() => onOpenSkills(workspace)}
      >
        <div className="workspaceCardBody">
          <div className="workspaceCardTitleRow">
            <strong>{workspace.displayName}</strong>
            <AgentIconBadge agent={workspace.agentIcon} />
          </div>
          <code className="workspaceCardPath">{workspace.compactPath}</code>
          {workspace.lastScanError ? <small>{workspace.lastScanError}</small> : null}
          <div className="workspaceCardMeta">
            {workspaceCardMetaLabels.map((label) => (
              <span className="workspaceCardMetric" key={label}>
                <small>{label}</small>
                {metaValues[label]}
              </span>
            ))}
          </div>
        </div>
      </button>
      {workspace.source === 'manual' ? (
        <button
          aria-label={`Forget ${workspace.compactPath}`}
          className="iconButton workspaceForgetButton"
          disabled={isBusy}
          type="button"
          onClick={() => onForget(workspace)}
        >
          <Trash2 aria-hidden="true" />
        </button>
      ) : null}
    </article>
  );
}

export function WorkspaceAddDialog({ dialog, status, onClose, onSubmit, onUpdate }) {
  const isBusy = status === 'scanning_workspaces';

  return (
    <div
      className="modalBackdrop"
      role="presentation"
      onMouseDown={(event) => closeOnBackdropClick(event, onClose)}
    >
      <section className="workspaceDialog" role="dialog" aria-modal="true" aria-labelledby="workspace-add-title">
        <div className="importSheetHeader">
          <div>
            <h2 id="workspace-add-title">Add workspace</h2>
            <p>Register an existing skills root.</p>
          </div>
          <button className="iconButton" disabled={isBusy} type="button" aria-label="Close workspace dialog" onClick={onClose}>
            <X aria-hidden="true" />
          </button>
        </div>

        <form className="remoteImportForm" onSubmit={onSubmit}>
          <label className="remoteImportField">
            <span>Path</span>
            <input
              autoFocus
              disabled={isBusy}
              placeholder="/path/to/.agents/skills"
              value={dialog.path}
              onChange={(event) => onUpdate({ path: event.target.value })}
            />
          </label>

          <div className="remoteImportModes" role="group" aria-label="Workspace scope">
            <button
              className={dialog.kind === 'user' ? 'active' : ''}
              disabled={isBusy}
              type="button"
              onClick={() => onUpdate({ kind: 'user' })}
            >
              User
            </button>
            <button
              className={dialog.kind === 'global' ? 'active' : ''}
              disabled={isBusy}
              type="button"
              onClick={() => onUpdate({ kind: 'global' })}
            >
              Global
            </button>
          </div>

          {dialog.error ? <div className="formError">{dialog.error}</div> : null}

          <div className="remoteImportFooter">
            <button className="button secondary" disabled={isBusy} type="button" onClick={onClose}>
              Cancel
            </button>
            <button className="button primary" disabled={isBusy} type="submit">
              {isBusy ? 'Scanning...' : 'Add workspace'}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}

export function DeployWorkspaceDialog({
  dialog,
  skill,
  status,
  onAddWorkspace,
  onClose,
  onConfirmUndeployChange,
  onSubmit,
  onToggleWorkspace
}) {
  const isBusy = status === 'deploying_skill';
  const changes = workspaceDeploymentChanges(dialog.rows);
  const requiresConfirmation = workspaceDeployRequiresConfirmation(changes);
  const changeCount = workspaceDeployChangeCount(changes);
  const canSubmit = changeCount > 0 && (!requiresConfirmation || dialog.confirmUndeploy);

  return (
    <div
      className="modalBackdrop"
      role="presentation"
      onMouseDown={(event) => closeOnBackdropClick(event, onClose)}
    >
      <section className="deployWorkspaceDialog" role="dialog" aria-modal="true" aria-labelledby="deploy-workspace-title">
        <div className="importSheetHeader">
          <div>
            <h2 id="deploy-workspace-title">Deploy to workspaces</h2>
            <p>{skill.name}</p>
          </div>
          <button className="iconButton" disabled={isBusy} type="button" aria-label="Close deploy workspace dialog" onClick={onClose}>
            <X aria-hidden="true" />
          </button>
        </div>

        <form className="deployWorkspaceForm" onSubmit={onSubmit}>
          <div className="deployWorkspaceToolbar">
            <div className="deployWorkspaceChangeSummary" aria-label="Pending deployment changes">
              <span>
                <Link2 aria-hidden="true" />
                {changes.deploy.length} link
              </span>
              <span>
                <Unlink aria-hidden="true" />
                {changes.undeploy.length} unlink
              </span>
            </div>
            <button className="button secondary" disabled={isBusy} type="button" onClick={onAddWorkspace}>
              <Plus aria-hidden="true" />
              Add workspace
            </button>
          </div>

          {dialog.rows.length > 0 ? (
            <div className="deployWorkspaceList" aria-label="Workspace deploy targets">
              {dialog.rows.map((workspace) => (
                <label className={workspace.isDeployed ? 'deployWorkspaceRow deployed' : 'deployWorkspaceRow'} key={workspace.canonicalPath || workspace.path}>
                  <input
                    aria-label={`Deploy ${skill.name} to workspace ${workspace.displayName}`}
                    checked={workspace.isSelected}
                    disabled={isBusy}
                    type="checkbox"
                    onChange={() => onToggleWorkspace(workspace.canonicalPath)}
                  />
                  <span className="deployWorkspaceCheck" aria-hidden="true">
                    <Check aria-hidden="true" />
                  </span>
                  <span className="deployWorkspaceMain">
                    <span className="deployWorkspaceTitle">
                      <strong>{workspace.displayName}</strong>
                      <Badge tone={workspace.kind === 'global' ? 'blue' : 'green'}>{workspace.kindLabel}</Badge>
                      {workspace.isDeployed ? <Badge tone="green">Linked</Badge> : null}
                    </span>
                    <code>{workspace.compactPath}</code>
                  </span>
                </label>
              ))}
            </div>
          ) : (
            <div className="deployWorkspaceEmpty">
              <strong>No workspaces registered</strong>
              <span>Add a workspace before deploying this skill.</span>
            </div>
          )}

          {requiresConfirmation ? (
            <div className="deployWorkspaceWarning">
              <AlertTriangle aria-hidden="true" />
              <div>
                <strong>Unchecked deployed workspaces will be unlinked.</strong>
                <span>SkillBox will remove only managed symlinks for {skill.name}; existing directories or foreign symlinks are refused.</span>
                <label>
                  <input
                    checked={dialog.confirmUndeploy}
                    disabled={isBusy}
                    type="checkbox"
                    onChange={(event) => onConfirmUndeployChange(event.target.checked)}
                  />
                  Confirm unlinking {changes.undeploy.length} workspace{changes.undeploy.length === 1 ? '' : 's'}
                </label>
              </div>
            </div>
          ) : null}

          {dialog.error ? <div className="formError">{dialog.error}</div> : null}

          <div className="remoteImportFooter">
            <button className="button secondary" disabled={isBusy} type="button" onClick={onClose}>
              Cancel
            </button>
            <button className="button primary" disabled={isBusy || !canSubmit} type="submit">
              {isBusy ? 'Updating...' : 'Apply deployment'}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}
