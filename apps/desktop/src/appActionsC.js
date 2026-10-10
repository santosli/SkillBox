import React, { useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import desktopPackage from '../package.json';
import skillBoxAppIcon from '../src-tauri/icons/icon.png';
import { FooterButton, NavButton } from './components/common.jsx';
import { Dashboard } from './components/dashboard.jsx';
import { HistoryPage } from './components/history.jsx';
import { UsageRankingsPage } from './components/rankings.jsx';
import {
  ImportReview,
  LocalImportConfirmationDialog,
  RemoteImportDialog
} from './components/importReview.jsx';
import {
  RemoteSourceBindingDialog,
  RemoteSourceCandidateBindDialog,
  RemoteVersionReviewDialog
} from './components/remoteSkills.jsx';
import { AppUpdateConfirmDialog, SettingsPage } from './components/settings.jsx';
import {
  ImportRevertDialog,
  SkillDeleteDialog,
  SkillDetailDialog,
  SkillTypeChangeDialog,
  CollectionRollbackDialog
} from './components/skillDetail.jsx';
import { UserSkillsSyncDialog } from './components/userSkillsSync.jsx';
import { UserSkillsInboundReviewDialog } from './components/userSkillsInbound.jsx';
import {
  DeployWorkspaceDialog,
  WorkspaceAddDialog,
  WorkspacePage
} from './components/workspaces.jsx';
import { skillMatchesDashboardFilters, sortDashboardSkills } from './dashboardFilters.js';
import {
  dashboardFilterOptions,
  deriveDashboardSkill,
  normalizeEditableTags
} from './dashboardMetadata.js';
import {
  historyRequestForFilter,
  isHistoryRequestCurrent,
  normalizeHistory
} from './historyEntries.js';
import {
  normalizeDoctorReport,
  normalizeStaleDeploymentRepairResult
} from './doctor.js';
import {
  normalizeImportCandidateGroups,
  normalizeImportCollections,
  normalizeGithubSkillCollectionPreviewResult,
  normalizeGithubSkillCollectionUpdatePreviewResult,
  attachGithubCollectionChanges,
  applyGithubCollectionChangeLocks,
  githubCollectionForSkill,
  normalizeImportCandidate,
  selectedImportCollectionRequests,
  selectedImportCandidates,
  selectImportCandidateVariant,
  toggleImportCollectionSelection,
  toggleImportCandidateGroup,
  toggleImportReviewSelection,
  updateImportCandidateGroupType,
  updateImportCollectionType
} from './importCandidates.js';
import {
  browserImportScanOptions,
  createImportScanRequestController,
  createRemoteImportRequestController,
  importScanCommandArgs,
  waitForImportScanDelay
} from './importScanProgress.js';
import {
  appUpdateNotice,
  appUpdateStatusAfterCheckError,
  normalizeAppUpdateStatus,
  previewAppUpdateStatus,
  shouldCheckAppUpdateOnStartup
} from './appUpdates.js';
import {
  importBatchNotice,
  importNotice,
  importRequestItems,
  isHttpUrl,
  isImportableCandidate,
  remoteImportCandidate,
  shouldConfirmLocalImport
} from './importFlow.js';
import {
  clearLegacyDashboardMetadata,
  normalizePreferences,
  previewCommitSummaryCliStorageKey,
  previewRemoteUpdateTimeoutStorageKey,
  previewStatusRefreshIntervalStorageKey,
  readDashboardFavorites,
  readDashboardTagOverrides,
  readPreviewPreferences
} from './preferences.js';
import {
  applyPreviewImportStatuses,
  candidateToPreviewSkill,
  previewCandidatesForWorkspace,
  previewHistory,
  previewImportCandidates,
  previewImportCandidateGroups,
  previewImportCollections,
  previewPaths,
  previewSkills,
  previewUsageRankings,
  previewUserSkillsGitChanges,
  previewUserSkillsInbound,
  previewUserSkillsInboundStatus,
  previewWorkspaces,
  publicPreviewRequested
} from './previewData.js';
import {
  normalizeRemoteSourceCandidates,
  normalizeRemoteSourceBindingPreview,
  normalizeRemoteInstallPreview,
  normalizeRemoteVersionPreview,
  remoteVersionActionLabel
} from './remoteSkills.js';
import {
  legacySkillUserMetadataUpdates,
  mergeSkillUserMetadataRow,
  normalizeSkillUserMetadata
} from './skillUserMetadata.js';
import {
  compactPath,
  defaultSkillStatus,
  hasAvailableUpdate,
  mergeSkills,
  normalizeOperationRecords,
  normalizePaths,
  normalizeRemoteSkillVersions,
  normalizeSkill
} from './skills.js';
import {
  dashboardStatusNotice,
  formatStatusCheckedAt,
  mergeRemoteSkillUpdates,
  normalizeRemoteSkillUpdates,
  normalizeStatusRefreshIntervalMinutes
} from './skillStatusRefresh.js';
import { normalizeUsageHookStatuses } from './usageHooks.js';
import { chooseWorkspaceDirectory } from './workspaceDirectoryPicker.js';
import {
  defaultUsageRankingFilters,
  normalizeUsageRankings,
  usageRankingRequest,
  normalizeCodexUsageBackfill,
  usageHistorySyncNotice,
  usageHistorySyncProviders
} from './usageRankings.js';
import {
  defaultSyncCommitMessage,
  normalizeSuggestedUserSkillsCommit,
  normalizeUserSkillsGitChanges,
  normalizeUserSkillsGitStatus,
  suggestUserSkillsCommitMessage,
  syncNotice,
  userSkillsSyncProgressSteps,
  waitForNextPaint
} from './userSkillsGitSync.js';
import {
  appendUserSkillsInboundWarnings,
  appliedUserSkillsInboundStatus,
  inboundApplyRefreshWarning,
  invalidateUserSkillsInboundPreview,
  normalizeUserSkillsInboundPreview,
  normalizeUserSkillsInboundStatus,
  useInboundReviewRequestController
} from './userSkillsInbound.js';
import {
  normalizeWorkspace,
  normalizeWorkspaceSetupPreview,
  normalizeWorkspaces,
  sidebarFooterItems,
  sidebarItems,
  workspaceCounts,
  workspaceDeployCanSubmit,
  workspaceDeployChangeCount,
  workspaceDeploymentChanges,
  workspaceDeployPickerRows,
  workspaceDeployRequiresConfirmation,
  workspaceMatchesFilters,
  workspaceSkillReviewMeta,
  workspaceTypeTabs
} from './workspaces.js';

const autoRefreshBlockedStatuses = new Set([
  'checking',
  'checking_health',
  'repairing_stale_deployments',
  'importing',
  'loading',
  'preparing_sync',
  'checking_inbound',
  'previewing_inbound',
  'applying_inbound',
  'deploying_skill',
  'deleting_skill',
  'installing_usage_hook',
  'loading_history',
  'scanning',
  'scanning_workspace_skills',
  'scanning_workspaces',
  'choosing_workspace',
  'previewing_workspace',
  'setting_up_workspace',
  'changing_skill_type',
  'reverting_import',
  'syncing'
]);

const closedRemoteSourceCandidateBind = {
  open: false,
  candidate: null,
  preview: null,
  loading: false,
  binding: false,
  error: ''
};

function prototypeWorkspaceSetupPreview(selectedPath, kind) {
  const path = selectedPath.replace(/\/$/, '');
  const exactRoot = kind === 'global' || path.endsWith('/skills');
  const detectedRootFixture = !exactRoot && path.endsWith('/multi-root-demo');
  const roots = exactRoot
    ? [{
        path,
        relative_path: 'skills',
        agent_id: 'custom',
        profile_id: 'custom-skill-md',
        profile_name: 'Custom SKILL.md',
        root_key: 'exact',
        format: 'skill_md',
        label: 'Custom SKILL.md',
        exists: true,
        recommended: true
      }]
    : [
        ['.agents/skills', 'agents', 'Agents'],
        ['.codex/skills', 'codex', 'Codex'],
        ['.claude/skills', 'claude-code', 'Claude Code'],
        ['.cursor/skills', 'cursor', 'Cursor']
      ].map(([relativePath, profileId, label], index) => ({
        path: `${path}/${relativePath}`,
        relative_path: relativePath,
        agent_id: profileId === 'claude-code' ? 'claude' : profileId,
        profile_id: profileId,
        profile_name: label,
        root_key: 'skills',
        format: 'skill_md',
        label,
        exists: detectedRootFixture && index < 2,
        recommended: index === 0
      }));
  return {
    preview_id: `prototype:${kind}:${path}`,
    selected_path: path,
    kind,
    mode: exactRoot
      ? 'existing_root'
      : detectedRootFixture
        ? 'project_with_roots'
        : 'project_without_roots',
    roots
  };
}

function normalizeImportRecord(record = {}) {
  const affectedDeploymentCount = Number(
    record.affectedDeploymentCount ?? record.affected_deployment_count
  );

  return {
    ...record,
    id: record.id || '',
    skillName: record.skillName || record.skill_name || '',
    kind: record.kind || record.type || 'user',
    sourcePath: record.sourcePath || record.source_path || '',
    sourceRoot: record.sourceRoot || record.source_root || '',
    managedPath: record.managedPath || record.managed_path || '',
    contentHash: record.contentHash || record.content_hash || '',
    backupPath: record.backupPath || record.backup_path || '',
    deployedPath: record.deployedPath || record.deployed_path || '',
    status: record.status || 'failed',
    legacy: Boolean(record.legacy),
    importedAt: record.importedAt || record.imported_at || '',
    revertedAt: record.revertedAt || record.reverted_at || '',
    canRevert: Boolean(record.canRevert ?? record.can_revert),
    revertBlockReason: record.revertBlockReason || record.revert_block_reason || '',
    affectedDeploymentCount: Number.isFinite(affectedDeploymentCount) ? affectedDeploymentCount : 0
  };
}

const publicPreview = import.meta.env.DEV
  && !window.__TAURI_INTERNALS__
  && publicPreviewRequested(window.location.search);
const APP_DISPLAY_NAME = import.meta.env.DEV && !publicPreview ? 'SkillBox Dev' : 'SkillBox';

export function createAppActions(getCtx) {
  function openSkill(skill) {
    const { loadImportRecords, loadRemoteSkillContext, loadUserSkillContext, setSelectedName } = getCtx();
    setSelectedName(skill.name);
    void loadImportRecords(skill.name);
    if (skill.type === 'remote') {
      void loadRemoteSkillContext(skill.name);
    } else if (skill.type === 'user') {
      void loadUserSkillContext(skill.name);
    }
  }

  function closeSkillDetail() {
    const { setSelectedName } = getCtx();
    setSelectedName('');
  }

  async function checkGithubCollectionUpdate(collection) {
    const { closeSkillDetail, setError, setImportReview, setNotice, setStatus } = getCtx();
    const sourceUrl = collection?.sourceUrl || collection?.source_url;
    if (!sourceUrl) {
      setError('This collection is missing a GitHub source URL.');
      return;
    }
    setStatus('checking');
    setError('');
    try {
      if (!window.__TAURI_INTERNALS__) {
        setNotice('Browser preview cannot check GitHub collection updates.');
        setStatus('prototype');
        return;
      }
      const updateResult = normalizeGithubSkillCollectionUpdatePreviewResult(
        await invoke('preview_github_skill_collection_update', {
          request: { source_url: sourceUrl }
        })
      );
      if (updateResult.kind === 'up_to_date') {
        setNotice(updateResult.message);
        setStatus('ready');
        return;
      }
      if (updateResult.kind !== 'update') {
        throw new Error(updateResult.message || 'Collection update preview could not continue.');
      }
      const updatePreview = updateResult.preview;
      const collections = normalizeImportCollections([{
        ...updatePreview.collection,
        from_sha: updatePreview.from_sha || updatePreview.fromSha,
        to_sha: updatePreview.to_sha || updatePreview.toSha,
        preview_id: updatePreview.preview_id || updatePreview.previewId
      }]).map((item) => attachGithubCollectionChanges(item, updatePreview.changes || []));
      const candidates = applyGithubCollectionChangeLocks(
        normalizeImportCandidateGroups(updatePreview.groups || [], []),
        collections
      );
      closeSkillDetail();
      setImportReview({
        open: true,
        loading: false,
        candidates,
        collections,
        errors: updatePreview.errors || updatePreview.collection?.errors || [],
        scanError: '',
        scanProgress: null,
        diagnostics: updatePreview.diagnostics || null,
        title: 'Collection update review',
        subtitle: 'Move this GitHub collection to one reviewed SHA. Updated members stay selected; nothing is auto-deployed.',
        noticePrefix: '',
        reviewMode: 'update',
        applyLabel: 'Apply collection update',
        applyingLabel: 'Updating...'
      });
      setStatus('ready');
    } catch (updateError) {
      setError(updateError.message || String(updateError) || 'Unable to check the collection update.');
      setStatus('ready');
    }
  }

  async function loadImportRecords(skillName) {
    const { setError, setImportRecordLoading, setImportRecords } = getCtx();
    if (!skillName) return;

    setImportRecordLoading((current) => ({ ...current, [skillName]: true }));

    if (!window.__TAURI_INTERNALS__) {
      setImportRecords((current) => ({ ...current, [skillName]: [] }));
      setImportRecordLoading((current) => ({ ...current, [skillName]: false }));
      return;
    }

    try {
      const result = await invoke('list_import_records', { skillName });
      setImportRecords((current) => ({
        ...current,
        [skillName]: (result.records || []).map(normalizeImportRecord)
      }));
    } catch (recordError) {
      setImportRecords((current) => ({ ...current, [skillName]: [] }));
      setError(recordError.message || String(recordError) || 'Unable to load import records.');
    } finally {
      setImportRecordLoading((current) => ({ ...current, [skillName]: false }));
    }
  }

  function closeImportRevertDialog() {
    const { error, importRevertDialog, setImportRevertDialog } = getCtx();
    if (importRevertDialog.loading) {
      return;
    }

    setImportRevertDialog({
      open: false,
      record: null,
      loading: false,
      error: ''
    });
  }

  async function openSkillDeleteDialog(skill) {
    const { error, setError, setNotice, setSkillDeleteDialog } = getCtx();
    if (!skill?.name) return;
    setError('');
    setNotice('');
    setSkillDeleteDialog({
      open: true,
      skillName: skill.name,
      preview: null,
      previewLoading: true,
      confirmation: '',
      loading: false,
      error: ''
    });

    if (!window.__TAURI_INTERNALS__) {
      setSkillDeleteDialog((current) => ({
        ...current,
        previewLoading: false,
        preview: {
          previewId: 'browser-preview',
          canDelete: true,
          deployments: skill.deployments || [],
          blockers: []
        }
      }));
      return;
    }

    try {
      const raw = await invoke('preview_delete_skill', { skillName: skill.name });
      setSkillDeleteDialog((current) =>
        current.open && current.skillName === skill.name
          ? {
              ...current,
              previewLoading: false,
              preview: {
                previewId: raw.previewId ?? raw.preview_id,
                canDelete: Boolean(raw.canDelete ?? raw.can_delete),
                deployments: raw.deployments || [],
                blockers: raw.blockers || []
              }
            }
          : current
      );
    } catch (deleteError) {
      setSkillDeleteDialog((current) =>
        current.open && current.skillName === skill.name
          ? {
              ...current,
              previewLoading: false,
              error: deleteError.message || String(deleteError) || 'Unable to review skill deletion.'
            }
          : current
      );
    }
  }

  function closeSkillDeleteDialog() {
    const { error, setSkillDeleteDialog, skillDeleteDialog } = getCtx();
    if (skillDeleteDialog.loading) return;
    setSkillDeleteDialog({
      open: false,
      skillName: '',
      preview: null,
      previewLoading: false,
      confirmation: '',
      loading: false,
      error: ''
    });
  }

  async function confirmSkillDelete() {
    const { closeSkillDeleteDialog, error, favoriteNames, filter, isFirstUse, paths, refresh, setDashboardTagOverrides, setFavoriteNames, setIsFirstUse, setNotice, setPaths, setRemoteSkillUpdates, setSelectedName, setSkillDeleteDialog, setSkills, setStatus, setUserSkillsGit, setWorkspaces, skillDeleteDialog, skills, userSkillsGit, workspaces } = getCtx();
    const { skillName, preview, confirmation } = skillDeleteDialog;
    if (!preview?.canDelete || confirmation !== skillName) return;
    setStatus('deleting_skill');
    setSkillDeleteDialog((current) => ({ ...current, loading: true, error: '' }));

    if (!window.__TAURI_INTERNALS__) {
      setSkills((current) => current.filter((skill) => skill.name !== skillName));
      setSelectedName('');
      closeSkillDeleteDialog();
      setNotice(`Deleted ${skillName} from SkillBox.`);
      setStatus('prototype');
      return;
    }

    try {
      const result = await invoke('delete_skill', {
        request: {
          skill_name: skillName,
          preview_id: preview.previewId,
          confirmed_skill_name: confirmation,
          actor: 'desktop'
        }
      });
      const removedCount = (result.removedDeployments ?? result.removed_deployments ?? []).length;
      setSkills((current) => current.filter((skill) => skill.name !== skillName));
      setRemoteSkillUpdates((current) => ({
        ...current,
        statuses: current.statuses.filter((item) => item.skillName !== skillName)
      }));
      setFavoriteNames((current) => current.filter((name) => name !== skillName));
      setDashboardTagOverrides((current) =>
        Object.fromEntries(Object.entries(current).filter(([name]) => name !== skillName))
      );
      setSelectedName('');
      setSkillDeleteDialog({ open: false, skillName: '', preview: null, previewLoading: false, confirmation: '', loading: false, error: '' });
      setStatus('ready');
      try {
        const [state, workspaceRows, gitStatus, metadataRows] = await Promise.all([
          invoke('managed_state'),
          invoke('list_workspaces').catch(() => workspaces),
          invoke('user_skills_git_status').catch(() => userSkillsGit),
          invoke('list_skill_user_metadata').catch(() => [])
        ]);
        setSkills(state.skills?.map(normalizeSkill) || []);
        setWorkspaces(normalizeWorkspaces(workspaceRows));
        setPaths(normalizePaths(state.paths));
        setIsFirstUse(Boolean(state.isFirstUse ?? state.is_first_use));
        setUserSkillsGit(normalizeUserSkillsGitStatus(gitStatus));
        const metadataState = normalizeSkillUserMetadata(metadataRows || []);
        setFavoriteNames(metadataState.favoriteNames);
        setDashboardTagOverrides(metadataState.tagOverrides);
        setNotice(`Deleted ${skillName} and removed it from ${removedCount} workspace${removedCount === 1 ? '' : 's'}.`);
      } catch (_refreshError) {
        setNotice(`Deleted ${skillName}, but the dashboard refresh failed. Reopen SkillBox to refresh managed state.`);
      }
    } catch (deleteError) {
      setSkillDeleteDialog((current) => ({
        ...current,
        loading: false,
        error: deleteError.message || String(deleteError) || 'Unable to delete skill.'
      }));
      setStatus('ready');
    }
  }

  function openDeployDialog(skill) {
    const { error, setDeployDialog, setError, setNotice, workspaces } = getCtx();
    setDeployDialog({
      open: true,
      skillName: skill.name,
      rows: workspaceDeployPickerRows(workspaces, skill.deployments || []),
      confirmUndeploy: false,
      error: ''
    });
    setError('');
    setNotice('');
  }

  function closeDeployDialog() {
    const { error, setDeployDialog, status } = getCtx();
    if (status === 'deploying_skill') {
      return;
    }
    setDeployDialog((current) => ({
      ...current,
      open: false,
      skillName: '',
      rows: [],
      confirmUndeploy: false,
      error: ''
    }));
  }

  async function toggleDeployWorkspace(canonicalPath) {
    const { deployDialog, error, setDeployDialog, status } = getCtx();
    const row = deployDialog.rows.find((item) => item.canonicalPath === canonicalPath);
    if (!row || row.compatibilityLoading) return;
    if (row.isSelected || row.isDeployed) {
      setDeployDialog((current) => ({
        ...current,
        rows: current.rows.map((item) =>
          item.canonicalPath === canonicalPath ? { ...item, isSelected: !item.isSelected } : item
        ),
        confirmUndeploy: false,
        error: ''
      }));
      return;
    }

    setDeployDialog((current) => ({
      ...current,
      rows: current.rows.map((item) =>
        item.canonicalPath === canonicalPath
          ? { ...item, compatibilityLoading: true, compatibilityError: '' }
          : item
      ),
      error: ''
    }));
    try {
      const compatibility = window.__TAURI_INTERNALS__
        ? await invoke('preview_skill_deployment', {
            request: {
              skill_name: deployDialog.skillName,
              target_root: row.path
            }
          })
        : row.profileId === 'agents'
          ? {
              preview_id: `prototype:${deployDialog.skillName}:${row.canonicalPath}`,
              status: 'warnings',
              issues: [{
                severity: 'warning',
                code: 'unknown_optional_frontmatter',
                message: 'Optional frontmatter field “author” will be preserved.',
                suggested_action: 'Review the field before deployment.'
              }],
              profile: { id: row.profileId, display_name: row.profileName }
            }
          : {
              preview_id: `prototype:${deployDialog.skillName}:${row.canonicalPath}`,
              status: 'compatible',
              issues: [],
              profile: { id: row.profileId, display_name: row.profileName }
            };
      setDeployDialog((current) => ({
        ...current,
        rows: current.rows.map((item) =>
          item.canonicalPath === canonicalPath
            ? {
                ...item,
                compatibility,
                compatibilityLoading: false,
                compatibilityError: '',
                isSelected: compatibility.status !== 'blocked',
                confirmWarnings: false
              }
            : item
        )
      }));
    } catch (previewError) {
      setDeployDialog((current) => ({
        ...current,
        rows: current.rows.map((item) =>
          item.canonicalPath === canonicalPath
            ? {
                ...item,
                compatibilityLoading: false,
                compatibilityError: previewError.message || String(previewError)
              }
            : item
        )
      }));
    }
  }

  async function loadRemoteSkillContext(skillName) {
    const { error, filter, setOperationHistory, setRemoteContextLoading, setRemoteSkillUpdates, setRemoteVersions, skills, status } = getCtx();
    if (!skillName) return;

    setRemoteContextLoading((current) => ({ ...current, [skillName]: true }));

    if (!window.__TAURI_INTERNALS__) {
      const mockLatestSha = '1234567890abcdef';
      setRemoteVersions((current) => ({
        ...current,
        [skillName]: normalizeRemoteSkillVersions({
          skill_name: skillName,
          current_version: 'manual-preview',
          versions: [
            {
              version: 'manual-preview',
              is_current: true,
              kind: 'manual',
              short_label: 'manual-preview',
              updated_at: Math.floor(Date.now() / 1000).toString()
            },
            {
              version: 'manual-previous',
              is_current: false,
              kind: 'manual',
              short_label: 'manual-previous',
              updated_at: Math.floor((Date.now() - 86400000) / 1000).toString()
            }
          ]
        })
      }));
      setRemoteSkillUpdates((current) =>
        normalizeRemoteSkillUpdates({
          statuses: [
            ...current.statuses.filter((status) => status.skillName !== skillName),
            {
              skill_name: skillName,
              source_type: 'github',
              current_version: 'manual-preview',
              source_url: `https://github.com/santosli/skillbox-preview/tree/main/remote-skills/${skillName}`,
              latest_sha: mockLatestSha,
              ref_kind: 'branch',
              tracking: true,
              update_available: true,
              state: 'update_available',
              message: 'Browser preview has a mock update available.'
            }
          ]
        })
      );
      setOperationHistory((current) => ({
        ...current,
        [skillName]: [
          {
            id: 'mock-failed-operation',
            operationType: 'bind_remote_source',
            status: 'failed',
            summary: 'Mock failed source binding.'
          }
        ]
      }));
      setRemoteContextLoading((current) => ({ ...current, [skillName]: false }));
      return;
    }

    try {
      const [versions, operations] = await Promise.all([
        invoke('list_remote_skill_versions', { skillName }),
        invoke('list_operations', {
          request: {
            entity_type: 'skill',
            entity_name: skillName,
            limit: 20
          }
        })
      ]);

      setRemoteVersions((current) => ({
        ...current,
        [skillName]: normalizeRemoteSkillVersions(versions)
      }));
      setOperationHistory((current) => ({
        ...current,
        [skillName]: normalizeOperationRecords(operations)
      }));
    } catch (contextError) {
      setOperationHistory((current) => ({
        ...current,
        [skillName]: [
          {
            id: 'context-error',
            operationType: 'load_remote_context',
            status: 'failed',
            summary: contextError.message || String(contextError)
          }
        ]
      }));
    } finally {
      setRemoteContextLoading((current) => ({ ...current, [skillName]: false }));
    }
  }

  async function openRemoteSourceDialog(skill) {
    const { error, searchRemoteSourceCandidates, setRemoteSourceDialog } = getCtx();
    setRemoteSourceDialog({
      open: true,
      skillName: skill.name,
      sourceUrl: '',
      candidates: [],
      searched: false,
      searching: true,
      searchError: '',
      preview: null,
      error: '',
      loading: false,
      binding: false,
      candidateBind: closedRemoteSourceCandidateBind
    });
    await waitForNextPaint();
    void searchRemoteSourceCandidates(skill.name);
  }

  function closeRemoteSourceDialog() {
    const { error, setRemoteSourceDialog } = getCtx();
    setRemoteSourceDialog((current) => ({
      ...current,
      open: false,
      error: '',
      loading: false,
      binding: false,
      candidateBind: closedRemoteSourceCandidateBind
    }));
  }

  function updateRemoteSourceDialog(patch) {
    const { error, setRemoteSourceDialog } = getCtx();
    setRemoteSourceDialog((current) => ({ ...current, ...patch, error: '' }));
  }

  async function loadRemoteSourceBindingPreview(skillName, sourceUrl) {
    const trimmedSourceUrl = sourceUrl.trim();
    if (!trimmedSourceUrl) {
      throw new Error('Enter or select a GitHub source URL.');
    }

    if (!window.__TAURI_INTERNALS__) {
      return normalizeRemoteSourceBindingPreview({
        skill_name: skillName,
        validation: 'same_skill_changed',
        current_version: 'manual-preview',
        latest_sha: '1234567890abcdef',
        ref_kind: 'branch',
        tracking: true,
        message: 'Skill names match but content differs. Binding will not replace current.'
      });
    }

    const result = await invoke('preview_remote_source_binding', {
      request: {
        skill_name: skillName,
        source_url: trimmedSourceUrl,
        actor: 'desktop'
      }
    });
    return normalizeRemoteSourceBindingPreview(result);
  }

  async function verifyAndBindRemoteSource(event) {
    const { error, loadRemoteSkillContext, loadRemoteSourceBindingPreview, refreshSkillStatuses, remoteSourceDialog, setNotice, setRemoteSourceDialog } = getCtx();
    event?.preventDefault?.();

    const trimmedSourceUrl = remoteSourceDialog.sourceUrl.trim();
    const skillName = remoteSourceDialog.skillName;

    if (!trimmedSourceUrl) {
      setRemoteSourceDialog((current) => ({ ...current, error: 'Enter or select a GitHub source URL.' }));
      return;
    }

    setRemoteSourceDialog((current) => ({
      ...current,
      sourceUrl: trimmedSourceUrl,
      loading: true,
      binding: false,
      preview: null,
      error: ''
    }));

    await waitForNextPaint();

    let preview;
    try {
      preview = await loadRemoteSourceBindingPreview(skillName, trimmedSourceUrl);
    } catch (previewError) {
      setRemoteSourceDialog((current) => ({
        ...current,
        loading: false,
        binding: false,
        error: previewError.message || String(previewError)
      }));
      return;
    }

    const verifiedSourceUrl = preview.sourceUrl || trimmedSourceUrl;

    if (preview.validation === 'mismatch') {
      setRemoteSourceDialog((current) => ({
        ...current,
        sourceUrl: verifiedSourceUrl,
        preview,
        loading: false,
        binding: false,
        error: preview.message || 'Source validation failed. Choose a GitHub source for this skill.'
      }));
      return;
    }

    setRemoteSourceDialog((current) => ({
      ...current,
      sourceUrl: verifiedSourceUrl,
      preview,
      loading: false,
      binding: true,
      error: ''
    }));

    await waitForNextPaint();

    if (!window.__TAURI_INTERNALS__) {
      setNotice(`Bound ${skillName} to GitHub source.`);
      setRemoteSourceDialog((current) => ({ ...current, open: false, loading: false, binding: false }));
      return;
    }

    try {
      await invoke('bind_remote_source', {
        request: {
          skill_name: skillName,
          source_url: verifiedSourceUrl,
          actor: 'desktop'
        }
      });
      setRemoteSourceDialog((current) => ({ ...current, open: false, loading: false, binding: false }));
      await refreshSkillStatuses();
      await loadRemoteSkillContext(skillName);
      setNotice(`Bound ${skillName} to GitHub source.`);
    } catch (bindError) {
      setRemoteSourceDialog((current) => ({
        ...current,
        loading: false,
        binding: false,
        error: bindError.message || String(bindError)
      }));
    }
  }

  async function bindRemoteSourceCandidate(candidate) {
    const { error, loadRemoteSourceBindingPreview, remoteSourceDialog, setRemoteSourceDialog } = getCtx();
    const sourceUrl = (candidate.sourceUrl || '').trim();
    const skillName = remoteSourceDialog.skillName;

    setRemoteSourceDialog((current) => ({
      ...current,
      sourceUrl,
      preview: null,
      error: '',
      candidateBind: {
        open: true,
        candidate: { ...candidate, sourceUrl },
        preview: null,
        loading: true,
        binding: false,
        error: ''
      }
    }));

    await waitForNextPaint();

    try {
      const preview = await loadRemoteSourceBindingPreview(skillName, sourceUrl);
      setRemoteSourceDialog((current) => {
        if (current.candidateBind.candidate?.sourceUrl !== sourceUrl) {
          return current;
        }

        return {
          ...current,
          sourceUrl: preview.sourceUrl || sourceUrl,
          candidateBind: {
            ...current.candidateBind,
            candidate: {
              ...current.candidateBind.candidate,
              path: preview.path || current.candidateBind.candidate?.path,
              sourceUrl: preview.sourceUrl || sourceUrl
            },
            preview,
            loading: false,
            error: ''
          }
        };
      });
    } catch (previewError) {
      setRemoteSourceDialog((current) => {
        if (current.candidateBind.candidate?.sourceUrl !== sourceUrl) {
          return current;
        }

        return {
          ...current,
          candidateBind: {
            ...current.candidateBind,
            preview: null,
            loading: false,
            error: previewError.message || String(previewError)
          }
        };
      });
    }
  }

  async function openRemoteVersionReview(skill, action, targetVersion = '') {
    const { error, setRemoteVersionDialog, status } = getCtx();
    setRemoteVersionDialog({
      open: true,
      loading: true,
      applying: false,
      preview: null,
      activePath: '',
      error: ''
    });

    await waitForNextPaint();

    if (!window.__TAURI_INTERNALS__) {
      const preview = normalizeRemoteVersionPreview({
        skill_name: skill.name,
        action,
        from_version: 'manual-preview',
        to_version: targetVersion || '1234567890abcdef',
        files: [
          {
            path: 'SKILL.md',
            status: 'M',
            diff: '@@\n-description: Old\n+description: New\n'
          }
        ]
      });
      setRemoteVersionDialog({
        open: true,
        loading: false,
        applying: false,
        preview,
        activePath: preview.activePath,
        error: ''
      });
      return;
    }

    try {
      const result = await invoke('preview_remote_version_change', {
        request: {
          skill_name: skill.name,
          action,
          target_version: targetVersion || null,
          actor: 'desktop'
        }
      });
      const preview = normalizeRemoteVersionPreview(result);
      setRemoteVersionDialog({
        open: true,
        loading: false,
        applying: false,
        preview,
        activePath: preview.activePath,
        error: ''
      });
    } catch (previewError) {
      setRemoteVersionDialog({
        open: true,
        loading: false,
        applying: false,
        preview: null,
        activePath: '',
        error: previewError.message || String(previewError)
      });
    }
  }

  function closeRemoteInstallDialog() {
    const { error, remoteImportRequestControllerRef, remoteInstallDialog, setRemoteInstallDialog } = getCtx();
    if (remoteInstallDialog.applying) return;
    remoteImportRequestControllerRef.current?.invalidate();
    setRemoteInstallDialog((current) => ({ ...current, open: false, error: '' }));
  }

  function updateRemoteInstallWarningConfirmation(confirmed) {
    const { error, setRemoteInstallDialog } = getCtx();
    setRemoteInstallDialog((current) => ({ ...current, confirmWarnings: confirmed, error: '' }));
  }

  async function toggleDashboardFavorite(skillName) {
    const { dashboardTagOverrides, favoriteNames, filter, setDashboardTagOverrides, setError, setFavoriteNames } = getCtx();
    const previous = favoriteNames;
    const favorite = !favoriteNames.includes(skillName);
    const next = favorite
      ? [...favoriteNames, skillName].sort((left, right) => left.localeCompare(right))
      : favoriteNames.filter((name) => name !== skillName);
    setFavoriteNames(next);

    if (!window.__TAURI_INTERNALS__) return;
    try {
      const persisted = await invoke('set_skill_user_metadata', {
        request: {
          skill_name: skillName,
          favorite,
          tags: dashboardTagOverrides[skillName] || []
        }
      });
      const authoritative = mergeSkillUserMetadataRow(
        next,
        dashboardTagOverrides,
        persisted
      );
      setFavoriteNames(authoritative.favoriteNames);
      setDashboardTagOverrides(authoritative.tagOverrides);
    } catch (metadataError) {
      setFavoriteNames(previous);
      setError(metadataError.message || String(metadataError));
    }
  }

  async function updateDashboardSkillTags(skillName, tags) {
    const { dashboardTagOverrides, favoriteNames, setDashboardTagOverrides, setError, setFavoriteNames } = getCtx();
    if (!skillName) {
      return;
    }

    const previous = dashboardTagOverrides;
    const normalizedTags = normalizeEditableTags(tags);
    const next = { ...dashboardTagOverrides, [skillName]: normalizedTags };
    setDashboardTagOverrides(next);

    if (!window.__TAURI_INTERNALS__) return;
    try {
      const persisted = await invoke('set_skill_user_metadata', {
        request: {
          skill_name: skillName,
          favorite: favoriteNames.includes(skillName),
          tags: normalizedTags
        }
      });
      const authoritative = mergeSkillUserMetadataRow(favoriteNames, next, persisted);
      setFavoriteNames(authoritative.favoriteNames);
      setDashboardTagOverrides(authoritative.tagOverrides);
    } catch (metadataError) {
      setDashboardTagOverrides(previous);
      setError(metadataError.message || String(metadataError));
    }
  }

  async function scanWorkspaceRegistry() {
    const { setError, setNotice, setStatus, setWorkspaces, workspaces } = getCtx();
    setStatus('scanning_workspaces');
    setError('');
    setNotice('');

    if (!window.__TAURI_INTERNALS__) {
      setWorkspaces(normalizeWorkspaces(previewWorkspaces));
      setNotice('Browser preview is using mock workspaces.');
      setStatus('prototype');
      return;
    }

    try {
      const result = await invoke('scan_workspaces');
      setWorkspaces(normalizeWorkspaces(result.workspaces || []));
      setNotice(
        result.error_count > 0
          ? `Scanned ${result.scanned_count} workspaces with ${result.error_count} issues.`
          : `Scanned ${result.scanned_count} workspaces.`
      );
      setStatus('ready');
    } catch (workspaceError) {
      setError(workspaceError.message || String(workspaceError) || 'Unable to scan workspaces.');
      setStatus('ready');
    }
  }

  function openWorkspaceDialog() {
    const { error, setError, setNotice, setWorkspaceDialog } = getCtx();
    setWorkspaceDialog({
      open: true,
      path: '',
      kind: 'user',
      error: '',
      preview: null,
      selectedRoot: ''
    });
    setNotice('');
    setError('');
  }

  function closeWorkspaceDialog() {
    const { error, setWorkspaceDialog, status } = getCtx();
    if (
      status === 'scanning_workspaces'
      || status === 'choosing_workspace'
      || status === 'previewing_workspace'
      || status === 'setting_up_workspace'
    ) {
      return;
    }
    setWorkspaceDialog((current) => ({ ...current, open: false, error: '' }));
  }

  function updateWorkspaceDialog(patch) {
    const { error, setWorkspaceDialog, workspacePreviewRequestRef } = getCtx();
    if ('path' in patch || 'kind' in patch) {
      workspacePreviewRequestRef.current += 1;
    }
    setWorkspaceDialog((current) => ({
      ...current,
      ...patch,
      error: '',
      ...(('path' in patch || 'kind' in patch) ? { preview: null, selectedRoot: '' } : {})
    }));
  }

  async function previewWorkspaceDialog(kindOverride, pathOverride) {
    const { error, filter, setStatus, setWorkspaceDialog, skills, workspaceDialog, workspacePreviewRequestRef } = getCtx();
    const workspacePath = (pathOverride ?? workspaceDialog.path).trim();
    const kind = kindOverride || workspaceDialog.kind;
    if (!workspacePath) {
      setWorkspaceDialog((current) => ({
        ...current,
        error: 'Enter a project or skills folder.',
        preview: null,
        selectedRoot: ''
      }));
      return null;
    }

    const requestId = workspacePreviewRequestRef.current + 1;
    workspacePreviewRequestRef.current = requestId;
    setStatus('previewing_workspace');
    setWorkspaceDialog((current) => ({
      ...current,
      path: pathOverride ?? current.path,
      kind,
      error: '',
      preview: null,
      selectedRoot: ''
    }));
    try {
      const rawPreview = window.__TAURI_INTERNALS__
        ? await invoke('preview_workspace_setup', {
            request: { selected_path: workspacePath, kind }
          })
        : prototypeWorkspaceSetupPreview(workspacePath, kind);
      const preview = normalizeWorkspaceSetupPreview(rawPreview);
      if (requestId !== workspacePreviewRequestRef.current) {
        return null;
      }
      const availableRoots = preview.mode === 'project_with_roots'
        ? preview.roots.filter((root) => root.exists)
        : preview.roots;
      const selected = availableRoots.find((root) => root.recommended) || availableRoots[0];
      setWorkspaceDialog((current) => ({
        ...current,
        kind,
        preview,
        selectedRoot: selected?.path || '',
        error: ''
      }));
      setStatus(window.__TAURI_INTERNALS__ ? 'ready' : 'prototype');
      return preview;
    } catch (workspaceError) {
      if (requestId !== workspacePreviewRequestRef.current) {
        return null;
      }
      setWorkspaceDialog((current) => ({
        ...current,
        error: workspaceError.message || String(workspaceError) || 'Unable to preview this folder.',
        preview: null,
        selectedRoot: ''
      }));
      setStatus(window.__TAURI_INTERNALS__ ? 'ready' : 'prototype');
      return null;
    }
  }

  async function forgetWorkspaceRow(workspace) {
    const { filter, setError, setNotice, setStatus, setWorkspaces } = getCtx();
    if (workspace.source !== 'manual') {
      return;
    }

    setStatus('scanning_workspaces');
    setError('');
    setNotice('');

    if (!window.__TAURI_INTERNALS__) {
      setWorkspaces((current) =>
        current.filter((item) => item.canonicalPath !== workspace.canonicalPath)
      );
      setNotice('Workspace forgotten.');
      setStatus('prototype');
      return;
    }

    try {
      const rows = await invoke('forget_workspace', { path: workspace.path });
      setWorkspaces(normalizeWorkspaces(rows));
      setNotice(`Workspace forgotten: ${workspace.compactPath}`);
      setStatus('ready');
    } catch (workspaceError) {
      setError(workspaceError.message || String(workspaceError) || 'Unable to forget workspace.');
      setStatus('ready');
    }
  }

  function openSyncSettings() {
    const { error, navigateToPage, setSyncDialog } = getCtx();
    setSyncDialog((current) => ({ ...current, open: false, error: '' }));
    navigateToPage('settings');
  }

  return {
    openSkill,
    closeSkillDetail,
    checkGithubCollectionUpdate,
    loadImportRecords,
    closeImportRevertDialog,
    openSkillDeleteDialog,
    closeSkillDeleteDialog,
    confirmSkillDelete,
    openDeployDialog,
    closeDeployDialog,
    toggleDeployWorkspace,
    loadRemoteSkillContext,
    openRemoteSourceDialog,
    closeRemoteSourceDialog,
    updateRemoteSourceDialog,
    loadRemoteSourceBindingPreview,
    verifyAndBindRemoteSource,
    bindRemoteSourceCandidate,
    openRemoteVersionReview,
    closeRemoteInstallDialog,
    updateRemoteInstallWarningConfirmation,
    toggleDashboardFavorite,
    updateDashboardSkillTags,
    scanWorkspaceRegistry,
    openWorkspaceDialog,
    closeWorkspaceDialog,
    updateWorkspaceDialog,
    previewWorkspaceDialog,
    forgetWorkspaceRow,
    openSyncSettings
  };
}
