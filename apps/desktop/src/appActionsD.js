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
  function navigateToPage(nextPage) {
    const { cancelUsageRankingRequest, history, historyRequestRef, pageRef, setPage } = getCtx();
    pageRef.current = nextPage;
    if (nextPage !== 'rankings') {
      cancelUsageRankingRequest();
    }
    if (nextPage !== 'history') {
      historyRequestRef.current += 1;
    }
    setPage(nextPage);
  }

  function openDashboard(nextFilter = getCtx().filter) {
    const { navigateToPage, setFilter, setSelectedName } = getCtx();
    setFilter(nextFilter);
    setSelectedName('');
    navigateToPage('dashboard');
  }

  function clearDashboardFilters() {
    const { setDashboardFavoritesOnly, setDashboardTagFilter, setFilter, setQuery } = getCtx();
    setQuery('');
    setFilter('all');
    setDashboardTagFilter('all');
    setDashboardFavoritesOnly(false);
  }

  function openHistory() {
    const { history, historyFilter, loadHistory, navigateToPage, setSelectedName } = getCtx();
    setSelectedName('');
    navigateToPage('history');
    void loadHistory(historyFilter);
  }

  async function loadHistory(nextFilter = getCtx().historyFilter) {
    const { history, historyRequestRef, setError, setHistory, setHistoryFilter, setStatus } = getCtx();
    const requestId = historyRequestRef.current + 1;
    historyRequestRef.current = requestId;
    setHistoryFilter(nextFilter);
    setError('');
    setHistory((current) => ({ ...current, entries: [] }));

    if (!window.__TAURI_INTERNALS__) {
      if (!isHistoryRequestCurrent(historyRequestRef.current, requestId)) return;
      setHistory(normalizeHistory(previewHistory(nextFilter)));
      setStatus('prototype');
      return;
    }

    setStatus('loading_history');
    try {
      const historyResult = await invoke('list_history', {
        request: historyRequestForFilter(nextFilter)
      });
      if (!isHistoryRequestCurrent(historyRequestRef.current, requestId)) return;
      setHistory(normalizeHistory(historyResult));
      setStatus('ready');
    } catch (historyError) {
      if (!isHistoryRequestCurrent(historyRequestRef.current, requestId)) return;
      setError(historyError.message || String(historyError) || 'Unable to load history.');
      setStatus('ready');
    }
  }

  function openRankings() {
    const { loadUsageRankings, navigateToPage, setSelectedName, usageRankingFilters } = getCtx();
    setSelectedName('');
    navigateToPage('rankings');
    void loadUsageRankings(usageRankingFilters);
  }

  function cancelUsageRankingRequest() {
    const { rankingImportRequestRef, setError, setRankingImportSkillName, setUsageRankingLoading, usageRankingRequestRef } = getCtx();
    usageRankingRequestRef.current += 1;
    rankingImportRequestRef.current += 1;
    setUsageRankingLoading(false);
    setRankingImportSkillName('');
    setError('');
  }

  async function loadUsageRankings(
    nextFilters,
    { clearError = true, reportError = true } = {}
  ) {
    const { pageRef, setError, setUsageRankingFilters, setUsageRankingLoading, setUsageRankings, usageRankingRequestRef } = getCtx();
    const requestId = usageRankingRequestRef.current + 1;
    usageRankingRequestRef.current = requestId;
    setUsageRankingFilters(nextFilters);
    setUsageRankingLoading(true);
    if (clearError) {
      setError('');
    }

    try {
      const result = window.__TAURI_INTERNALS__
        ? await invoke('list_skill_usage_rankings', {
            request: usageRankingRequest(nextFilters)
          })
        : previewUsageRankings(nextFilters);
      if (usageRankingRequestRef.current === requestId && pageRef.current === 'rankings') {
        setUsageRankings(normalizeUsageRankings(result));
      }
      return '';
    } catch (rankingError) {
      const rankingErrorMessage =
        rankingError.message || String(rankingError) || 'Unable to load skill usage rankings.';
      if (
        reportError
        && usageRankingRequestRef.current === requestId
        && pageRef.current === 'rankings'
      ) {
        setError(rankingErrorMessage);
      }
      return rankingErrorMessage;
    } finally {
      if (usageRankingRequestRef.current === requestId) {
        setUsageRankingLoading(false);
      }
    }
  }

  function openRankedSkill(skillName) {
    const { openSkill, setError, skills } = getCtx();
    const skill = skills.find((candidate) => candidate.name === skillName);
    if (!skill) {
      setError(`Managed skill ${skillName} was not found. Refresh Rankings and try again.`);
      return;
    }
    openSkill(skill);
  }

  async function importRankedSkill(row) {
    const { pageRef, rankingImportRequestRef, setError, setLocalImportConfirmation, setNotice, setRankingImportSkillName, skills, usageRankingFilters, usageRankings } = getCtx();
    if (pageRef.current !== 'rankings') return;
    const skillName = row.skillName;
    const sourceId = row.sourceId || skillName;
    const requestId = rankingImportRequestRef.current + 1;
    rankingImportRequestRef.current = requestId;
    setRankingImportSkillName(sourceId);
    setError('');
    setNotice('');

    try {
      const candidate = window.__TAURI_INTERNALS__
        ? normalizeImportCandidate(
            await invoke('preview_usage_skill_import', {
              request: {
                skillName,
                sourceKind: row.sourceKind || (row.system ? 'system' : 'regular'),
                sourceId: row.sourceId || null,
                sourceRuntimeRoots: row.sourceRuntimeRoots || [],
                rankingRequest: usageRankingRequest(usageRankingFilters),
                rankingGeneratedAt: usageRankings.generatedAt
              }
            })
          )
        : normalizeImportCandidate({
            name: skillName,
            description: `Preview import for ${skillName}`,
            sourcePath: `/tmp/preview-skills/${skillName}`,
            sourceRoot: '/tmp/preview-skills',
            realPath: `/tmp/preview-skills/${skillName}`,
            isSymlink: false,
            contentHash: `preview-${skillName}`,
            suggestedType: 'user',
            suggestionReason: 'Observed in Rankings',
            importStatus: 'importable',
            isSelected: true,
            usageCount: 1
          });

      if (!isImportableCandidate(candidate)) {
        throw new Error(
          candidate.conflict
            || `Skill ${skillName} is not importable from the recorded runtime location.`
        );
      }

      if (
        rankingImportRequestRef.current !== requestId
        || pageRef.current !== 'rankings'
      ) return;
      setLocalImportConfirmation({
        open: true,
        candidates: [candidate],
        noticePrefix: 'Imported from Rankings.'
      });
    } catch (importError) {
      if (
        rankingImportRequestRef.current !== requestId
        || pageRef.current !== 'rankings'
      ) return;
      setError(
        importError.message
          || String(importError)
          || `Unable to prepare import for ${skillName}.`
      );
    } finally {
      if (rankingImportRequestRef.current === requestId) {
        setRankingImportSkillName('');
      }
    }
  }

  async function openGithubCollectionRollback(collection) {
    const { error, setCollectionRollbackDialog } = getCtx();
    const collectionId = collection?.id;
    if (!collectionId) {
      return;
    }
    setCollectionRollbackDialog({
      open: true,
      loading: true,
      applying: false,
      preview: null,
      error: ''
    });
    try {
      if (!window.__TAURI_INTERNALS__) {
        throw new Error('Browser preview cannot roll back GitHub collections.');
      }
      const preview = await invoke('preview_github_skill_collection_rollback', {
        request: {
          collection_id: collectionId,
          preview_id: '',
          actor: 'desktop'
        }
      });
      setCollectionRollbackDialog({
        open: true,
        loading: false,
        applying: false,
        preview,
        error: (preview.errors || []).join(' ')
      });
    } catch (rollbackError) {
      setCollectionRollbackDialog({
        open: true,
        loading: false,
        applying: false,
        preview: null,
        error: rollbackError.message || String(rollbackError) || 'Unable to preview collection rollback.'
      });
    }
  }

  async function applyGithubCollectionRollback() {
    const { closeSkillDetail, collectionRollbackDialog, error, refresh, setCollectionRollbackDialog, setNotice } = getCtx();
    const preview = collectionRollbackDialog.preview;
    if (!preview) {
      return;
    }
    setCollectionRollbackDialog((current) => ({ ...current, applying: true, error: '' }));
    try {
      await invoke('apply_github_skill_collection_rollback', {
        request: {
          collection_id: preview.collection_id || preview.collectionId,
          preview_id: preview.preview_id || preview.previewId,
          actor: 'desktop'
        }
      });
      setCollectionRollbackDialog({
        open: false,
        loading: false,
        applying: false,
        preview: null,
        error: ''
      });
      closeSkillDetail();
      await refresh();
      setNotice('Rolled the GitHub collection back to the previous reviewed SHA. Deployments were not changed automatically.');
    } catch (rollbackError) {
      setCollectionRollbackDialog((current) => ({
        ...current,
        applying: false,
        error: rollbackError.message || String(rollbackError) || 'Unable to roll back this collection.'
      }));
    }
  }

  function openImportRevertDialog(record) {
    const { error, setError, setImportRevertDialog, setNotice } = getCtx();
    if (!record?.canRevert) {
      return;
    }

    setImportRevertDialog({
      open: true,
      record,
      loading: false,
      error: ''
    });
    setError('');
    setNotice('');
  }

  async function confirmImportRevert() {
    const { error, importRevertDialog, isFirstUse, paths, setError, setImportRecords, setImportRevertDialog, setIsFirstUse, setNotice, setPaths, setSelectedName, setSkills, setStatus, setWorkspaces, skills, status, workspaces } = getCtx();
    const record = importRevertDialog.record;
    if (!record?.id) {
      return;
    }

    setStatus('reverting_import');
    setError('');
    setNotice('');
    setImportRevertDialog((current) => ({ ...current, loading: true, error: '' }));

    if (!window.__TAURI_INTERNALS__) {
      setImportRecords((current) => ({
        ...current,
        [record.skillName]: (current[record.skillName] || []).map((item) =>
          item.id === record.id ? { ...item, status: 'reverted', canRevert: false } : item
        )
      }));
      setImportRevertDialog({ open: false, record: null, loading: false, error: '' });
      setSelectedName('');
      setNotice(`Reverted import for ${record.skillName}.`);
      setStatus('prototype');
      return;
    }

    try {
      await invoke('revert_import', {
        request: {
          import_record_id: record.id,
          actor: 'desktop'
        }
      });
      const [state, workspaceRows, recordRows] = await Promise.all([
        invoke('managed_state'),
        invoke('list_workspaces').catch(() => workspaces),
        invoke('list_import_records', { skillName: record.skillName }).catch(() => ({ records: [] }))
      ]);
      const managedSkills = state.skills?.map(normalizeSkill) || [];

      setSkills(managedSkills);
      setWorkspaces(normalizeWorkspaces(workspaceRows));
      setPaths(normalizePaths(state.paths));
      setIsFirstUse(Boolean(state.isFirstUse ?? state.is_first_use));
      setSelectedName('');
      setImportRecords((current) => ({
        ...current,
        [record.skillName]: (recordRows.records || []).map(normalizeImportRecord)
      }));
      setImportRevertDialog({ open: false, record: null, loading: false, error: '' });
      setNotice(`Reverted import for ${record.skillName}.`);
      setStatus('ready');
    } catch (revertError) {
      setImportRevertDialog((current) => ({
        ...current,
        loading: false,
        error: revertError.message || String(revertError) || 'Unable to revert import.'
      }));
      setStatus('ready');
    }
  }

  function openSkillTypeChangeDialog(skill, targetType) {
    const { error, setError, setNotice, setSkillTypeChangeDialog } = getCtx();
    if (!skill || skill.type === targetType) {
      return;
    }

    setSkillTypeChangeDialog({
      open: true,
      skillName: skill.name,
      currentType: skill.type,
      targetType,
      loading: false,
      error: ''
    });
    setError('');
    setNotice('');
  }

  function closeSkillTypeChangeDialog() {
    const { error, setSkillTypeChangeDialog, skillTypeChangeDialog } = getCtx();
    if (skillTypeChangeDialog.loading) {
      return;
    }

    setSkillTypeChangeDialog({
      open: false,
      skillName: '',
      currentType: '',
      targetType: '',
      loading: false,
      error: ''
    });
  }

  async function confirmSkillTypeChange() {
    const { error, isFirstUse, loadRemoteSkillContext, loadUserSkillContext, paths, remoteSkillUpdates, setError, setIsFirstUse, setNotice, setPaths, setRemoteSkillUpdates, setSelectedName, setSkillTypeChangeDialog, setSkills, setStatus, setUserSkillsGit, setWorkspaces, skillTypeChangeDialog, skills, status, workspaces } = getCtx();
    if (!skillTypeChangeDialog.open || !skillTypeChangeDialog.skillName || !skillTypeChangeDialog.targetType) {
      return;
    }

    const skillName = skillTypeChangeDialog.skillName;
    const targetType = skillTypeChangeDialog.targetType;
    setStatus('changing_skill_type');
    setError('');
    setNotice('');
    setSkillTypeChangeDialog((current) => ({ ...current, loading: true, error: '' }));

    if (!window.__TAURI_INTERNALS__) {
      setSkills((current) =>
        current.map((skill) =>
          skill.name === skillName
            ? { ...skill, type: targetType, status: defaultSkillStatus(targetType) }
            : skill
        )
      );
      setSkillTypeChangeDialog({
        open: false,
        skillName: '',
        currentType: '',
        targetType: '',
        loading: false,
        error: ''
      });
      if (targetType === 'remote') {
        void loadRemoteSkillContext(skillName);
      } else {
        void loadUserSkillContext(skillName);
      }
      setNotice(`Changed ${skillName} to ${targetType} skill.`);
      setStatus('prototype');
      return;
    }

    try {
      await invoke('change_skill_kind', {
        skillName: skillTypeChangeDialog.skillName,
        skillType: skillTypeChangeDialog.targetType
      });
      const [state, gitStatus, workspaceRows, cachedRemoteUpdatesResult] = await Promise.all([
        invoke('managed_state'),
        invoke('user_skills_git_status').catch(() => null),
        invoke('list_workspaces').catch(() => workspaces),
        invoke('cached_remote_skill_updates').catch(() => remoteSkillUpdates)
      ]);
      const managedSkills = state.skills?.map(normalizeSkill) || [];

      setSkills(managedSkills);
      setWorkspaces(normalizeWorkspaces(workspaceRows));
      setPaths(normalizePaths(state.paths));
      setUserSkillsGit(normalizeUserSkillsGitStatus(gitStatus));
      setRemoteSkillUpdates(normalizeRemoteSkillUpdates(cachedRemoteUpdatesResult));
      setIsFirstUse(Boolean(state.isFirstUse ?? state.is_first_use));
      setSelectedName((currentName) =>
        currentName && managedSkills.some((skill) => skill.name === currentName) ? currentName : ''
      );
      setSkillTypeChangeDialog({
        open: false,
        skillName: '',
        currentType: '',
        targetType: '',
        loading: false,
        error: ''
      });
      if (targetType === 'remote') {
        void loadRemoteSkillContext(skillName);
      } else {
        void loadUserSkillContext(skillName);
      }
      setNotice(`Changed ${skillName} to ${targetType} skill.`);
      setStatus('ready');
    } catch (typeError) {
      setSkillTypeChangeDialog((current) => ({
        ...current,
        loading: false,
        error: typeError.message || String(typeError) || 'Unable to change skill type.'
      }));
      setStatus('ready');
    }
  }

  function updateDeployWarningConfirmation(canonicalPath, confirmed) {
    const { error, setDeployDialog } = getCtx();
    setDeployDialog((current) => ({
      ...current,
      rows: current.rows.map((row) =>
        row.canonicalPath === canonicalPath ? { ...row, confirmWarnings: confirmed } : row
      ),
      error: ''
    }));
  }

  function updateDeployUndeployConfirmation(confirmed) {
    const { error, setDeployDialog } = getCtx();
    setDeployDialog((current) => ({
      ...current,
      confirmUndeploy: confirmed,
      error: ''
    }));
  }

  function refreshDeployDialogRows(nextWorkspaces) {
    const { error, filter, setDeployDialog } = getCtx();
    setDeployDialog((current) => {
      if (!current.open) {
        return current;
      }

      const selectedByPath = new Map(
        current.rows.map((row) => [row.canonicalPath || row.path, row.isSelected])
      );
      const deployedRows = current.rows
        .filter((row) => row.isDeployed)
        .map((row) => ({ target_root: row.path }));
      const rows = workspaceDeployPickerRows(nextWorkspaces, deployedRows).map((row) => {
        const key = row.canonicalPath || row.path;
        const previous = current.rows.find((item) => (item.canonicalPath || item.path) === key);
        return selectedByPath.has(key)
          ? {
              ...row,
              isSelected: selectedByPath.get(key),
              compatibility: previous?.compatibility || null,
              confirmWarnings: Boolean(previous?.confirmWarnings)
            }
          : row;
      });

      return { ...current, rows, confirmUndeploy: false, error: '' };
    });
  }

  async function submitDeployDialog(event) {
    const { closeDeployDialog, deployDialog, error, filter, isFirstUse, paths, setDeployDialog, setError, setIsFirstUse, setNotice, setPaths, setSelectedName, setSkills, setStatus, setWorkspaces, skills, workspaces } = getCtx();
    event.preventDefault();
    const changes = workspaceDeploymentChanges(deployDialog.rows);
    const changeCount = workspaceDeployChangeCount(changes);
    const needsUndeployConfirmation = workspaceDeployRequiresConfirmation(changes);

    if (changeCount === 0) {
      closeDeployDialog();
      return;
    }
    if (needsUndeployConfirmation && !deployDialog.confirmUndeploy) {
      setDeployDialog((current) => ({
        ...current,
        error: 'Confirm unlinking before applying these deployment changes.'
      }));
      return;
    }
    if (!workspaceDeployCanSubmit(deployDialog.rows)) {
      setDeployDialog((current) => ({
        ...current,
        error: 'Resolve blocked targets and confirm compatibility warnings before deploying.'
      }));
      return;
    }

    setStatus('deploying_skill');
    setError('');
    setNotice('');

    if (!window.__TAURI_INTERNALS__) {
      const nextDeployments = deployDialog.rows
        .filter((row) => row.isSelected)
        .map((row) => ({
          target_root: row.path,
          target_path: `${row.path}/${deployDialog.skillName}`,
          mode: 'symlink'
        }));
      setSkills((current) =>
        current.map((skill) =>
          skill.name === deployDialog.skillName ? { ...skill, deployments: nextDeployments } : skill
        )
      );
      setDeployDialog({ open: false, skillName: '', rows: [], confirmUndeploy: false, error: '' });
      setNotice(`Updated deployments: ${changes.deploy.length} linked, ${changes.undeploy.length} unlinked.`);
      setStatus('prototype');
      return;
    }

    try {
      for (const workspace of changes.deploy) {
        await invoke('apply_skill_deployment', {
          request: {
            skill_name: deployDialog.skillName,
            target_root: workspace.path,
            preview_id: workspace.compatibility?.preview_id,
            confirm_warnings: Boolean(workspace.confirmWarnings)
          }
        });
      }
      for (const workspace of changes.undeploy) {
        await invoke('undeploy_skill', {
          skillName: deployDialog.skillName,
          targetRoot: workspace.path
        });
      }

      const [state, workspaceRows] = await Promise.all([
        invoke('managed_state'),
        invoke('list_workspaces').catch(() => workspaces)
      ]);
      const managedSkills = state.skills?.map(normalizeSkill) || [];
      const normalizedWorkspaces = normalizeWorkspaces(workspaceRows);

      setSkills(managedSkills);
      setWorkspaces(normalizedWorkspaces);
      setPaths(normalizePaths(state.paths));
      setIsFirstUse(Boolean(state.isFirstUse ?? state.is_first_use));
      setSelectedName((currentName) =>
        currentName && managedSkills.some((skill) => skill.name === currentName) ? currentName : ''
      );
      setDeployDialog({ open: false, skillName: '', rows: [], confirmUndeploy: false, error: '' });
      setNotice(`Updated deployments: ${changes.deploy.length} linked, ${changes.undeploy.length} unlinked.`);
      setStatus('ready');
    } catch (deployError) {
      setDeployDialog((current) => ({
        ...current,
        error: deployError.message || String(deployError) || 'Unable to update deployments.'
      }));
      setStatus('ready');
    }
  }

  async function loadUserSkillContext(skillName) {
    const { setUserContextLoading, setUserVersions } = getCtx();
    if (!skillName) return;

    setUserContextLoading((current) => ({ ...current, [skillName]: true }));

    if (!window.__TAURI_INTERNALS__) {
      setUserVersions((current) => ({
        ...current,
        [skillName]: normalizeRemoteSkillVersions({
          skill_name: skillName,
          current_version: 'preview-working',
          versions: [
            {
              version: 'preview-working',
              is_current: true,
              kind: 'working',
              short_label: 'preview-working',
              updated_at: Math.floor(Date.now() / 1000).toString()
            },
            {
              version: 'abcdef1234567890',
              is_current: false,
              kind: 'git',
              short_label: 'abcdef123456',
              updated_at: Math.floor((Date.now() - 86400000) / 1000).toString(),
              message: 'Preview user skill commit'
            }
          ]
        })
      }));
      setUserContextLoading((current) => ({ ...current, [skillName]: false }));
      return;
    }

    try {
      const versions = await invoke('list_user_skill_versions', { skillName });
      setUserVersions((current) => ({
        ...current,
        [skillName]: normalizeRemoteSkillVersions(versions)
      }));
    } catch (contextError) {
      setUserVersions((current) => ({
        ...current,
        [skillName]: normalizeRemoteSkillVersions({
          skill_name: skillName,
          current_version: '',
          versions: []
        })
      }));
    } finally {
      setUserContextLoading((current) => ({ ...current, [skillName]: false }));
    }
  }

  async function searchRemoteSourceCandidates(skillName) {
    const { setRemoteSourceDialog, skills } = getCtx();
    if (!skillName) return;

    setRemoteSourceDialog((current) =>
      current.skillName === skillName
        ? { ...current, searching: true, searched: false, searchError: '', candidates: [] }
        : current
    );

    if (!window.__TAURI_INTERNALS__) {
      const search = normalizeRemoteSourceCandidates({
        skill_name: skillName,
        candidates: [
          {
            owner: 'santosli',
            repo: 'skillbox-preview',
            path: `remote-skills/${skillName}`,
            reference: 'main',
            source_url: `https://github.com/santosli/skillbox-preview/tree/main/remote-skills/${skillName}`,
            repo_url: 'https://github.com/santosli/skillbox-preview.git',
            name: skillName,
            description: 'Mock GitHub source candidate for browser preview.',
            stars: 12,
            archived: false,
            fork: false,
            updated_at: new Date().toISOString(),
            match_reasons: ['Exact skill name match'],
            score: 570
          }
        ]
      });
      setRemoteSourceDialog((current) =>
        current.skillName === skillName
          ? { ...current, candidates: search.candidates, searching: false, searched: true }
          : current
      );
      return;
    }

    try {
      const result = await invoke('find_remote_source_candidates', { skillName });
      const search = normalizeRemoteSourceCandidates(result);
      setRemoteSourceDialog((current) =>
        current.skillName === skillName
          ? { ...current, candidates: search.candidates, searching: false, searched: true }
          : current
      );
    } catch (searchError) {
      setRemoteSourceDialog((current) =>
        current.skillName === skillName
          ? {
              ...current,
              candidates: [],
              searching: false,
              searched: true,
              searchError: searchError.message || String(searchError)
            }
          : current
      );
    }
  }

  async function viewRemoteSourceCandidate(candidate) {
    const { error, setRemoteSourceDialog } = getCtx();
    const sourceUrl = (candidate.sourceUrl || '').trim();
    if (!sourceUrl) return;

    if (window.__TAURI_INTERNALS__) {
      try {
        await invoke('open_external_url', { url: sourceUrl });
        return;
      } catch (viewError) {
        setRemoteSourceDialog((current) => ({
          ...current,
          error: viewError.message || String(viewError)
        }));
      }
    }

    window.open(sourceUrl, '_blank', 'noopener,noreferrer');
  }

  async function openRemoteSourceUrl(sourceUrl) {
    const { setNotice } = getCtx();
    const url = (sourceUrl || '').trim();
    if (!url) return;

    if (window.__TAURI_INTERNALS__) {
      try {
        await invoke('open_external_url', { url });
        return;
      } catch (viewError) {
        setNotice(viewError.message || String(viewError));
      }
    }

    window.open(url, '_blank', 'noopener,noreferrer');
  }

  async function openLocalSkillFolder(skill) {
    const { setNotice } = getCtx();
    const folderPath = String(skill?.path || '').trim();
    if (!folderPath) {
      setNotice('No local skill folder is available for this skill.');
      return;
    }

    if (window.__TAURI_INTERNALS__) {
      try {
        await invoke('open_local_path', { path: folderPath });
        return;
      } catch (viewError) {
        setNotice(viewError.message || String(viewError));
        return;
      }
    }

    setNotice(`Local folder: ${compactPath(folderPath)}`);
  }

  function closeRemoteSourceCandidateBind() {
    const { setRemoteSourceDialog } = getCtx();
    setRemoteSourceDialog((current) => ({
      ...current,
      candidateBind: closedRemoteSourceCandidateBind
    }));
  }

  async function confirmRemoteSourceCandidateBind() {
    const { error, loadRemoteSkillContext, refreshSkillStatuses, remoteSourceDialog, setNotice, setRemoteSourceDialog } = getCtx();
    const candidateBind = remoteSourceDialog.candidateBind;
    const candidate = candidateBind.candidate;
    const sourceUrl = (candidate?.sourceUrl || '').trim();
    const preview = candidateBind.preview;
    const skillName = remoteSourceDialog.skillName;

    if (!sourceUrl || !preview || preview.validation === 'mismatch' || candidateBind.loading || candidateBind.binding) {
      return;
    }

    setRemoteSourceDialog((current) => ({
      ...current,
      candidateBind: {
        ...current.candidateBind,
        binding: true,
        error: ''
      }
    }));

    if (!window.__TAURI_INTERNALS__) {
      setNotice(`Bound ${skillName} to GitHub source.`);
      setRemoteSourceDialog((current) => ({
        ...current,
        open: false,
        loading: false,
        candidateBind: closedRemoteSourceCandidateBind
      }));
      return;
    }

    try {
      await invoke('bind_remote_source', {
        request: {
          skill_name: skillName,
          source_url: sourceUrl,
          actor: 'desktop'
        }
      });
      setRemoteSourceDialog((current) => ({
        ...current,
        open: false,
        loading: false,
        candidateBind: closedRemoteSourceCandidateBind
      }));
      await refreshSkillStatuses();
      await loadRemoteSkillContext(skillName);
      setNotice(`Bound ${skillName} to GitHub source.`);
    } catch (bindError) {
      setRemoteSourceDialog((current) => ({
        ...current,
        candidateBind: {
          ...current.candidateBind,
          binding: false,
          error: bindError.message || String(bindError)
        }
      }));
    }
  }

  function closeRemoteVersionDialog() {
    const { error, remoteVersionDialog, setRemoteVersionDialog } = getCtx();
    if (remoteVersionDialog.applying) return;
    setRemoteVersionDialog((current) => ({ ...current, open: false, error: '' }));
  }

  function activateRemoteVersionPath(path) {
    const { setRemoteVersionDialog } = getCtx();
    setRemoteVersionDialog((current) => ({ ...current, activePath: path }));
  }

  async function applyRemoteVersionChange() {
    const { error, loadRemoteSkillContext, refreshSkillStatuses, remoteVersionDialog, setNotice, setRemoteVersionDialog } = getCtx();
    const preview = remoteVersionDialog.preview;
    if (!preview) return;
    setRemoteVersionDialog((current) => ({ ...current, applying: true, error: '' }));

    if (!window.__TAURI_INTERNALS__) {
      setNotice(`${remoteVersionActionLabel(preview)} applied for ${preview.skillName}.`);
      setRemoteVersionDialog((current) => ({ ...current, open: false, applying: false }));
      return;
    }

    try {
      await invoke('apply_remote_version_change', {
        request: {
          skill_name: preview.skillName,
          action: preview.action,
          target_version: preview.toVersion,
          preview_id: preview.previewId || null,
          actor: 'desktop'
        }
      });
      setRemoteVersionDialog((current) => ({ ...current, open: false, applying: false }));
      await refreshSkillStatuses({ skillName: preview.skillName });
      await loadRemoteSkillContext(preview.skillName);
      setNotice(`${remoteVersionActionLabel(preview)} applied for ${preview.skillName}.`);
    } catch (applyError) {
      setRemoteVersionDialog((current) => ({
        ...current,
        applying: false,
        error: applyError.message || String(applyError)
      }));
    }
  }

  function activateRemoteInstallPath(path) {
    const { setRemoteInstallDialog } = getCtx();
    setRemoteInstallDialog((current) => ({ ...current, activePath: path }));
  }

  async function applyRemoteInstall() {
    const { error, refresh, remoteInstallDialog, setNotice, setRemoteInstallDialog } = getCtx();
    const preview = remoteInstallDialog.preview;
    if (!preview) return;
    setRemoteInstallDialog((current) => ({ ...current, applying: true, error: '' }));

    if (!window.__TAURI_INTERNALS__) {
      setNotice(`Installed ${preview.skillName} from GitHub.`);
      setRemoteInstallDialog((current) => ({ ...current, open: false, applying: false }));
      return;
    }

    try {
      const result = await invoke('install_github_remote_skill', {
        request: {
          source_url: preview.sourceUrl,
          target_root: preview.targetRoot || null,
          preview_id: preview.previewId || null,
          confirm_warnings: Boolean(remoteInstallDialog.confirmWarnings),
          actor: 'desktop'
        }
      });
      setRemoteInstallDialog((current) => ({ ...current, open: false, applying: false }));
      await refresh();
      setNotice(`Installed ${result.skillName || result.skill_name || preview.skillName || 'remote skill'} from GitHub.`);
    } catch (installError) {
      setRemoteInstallDialog((current) => ({
        ...current,
        applying: false,
        error: installError.message || String(installError)
      }));
    }
  }

  async function saveUserSkillsGitRemote(remoteUrl) {
    const { authoritativeGenerationRef, setNotice, setStatus, setUserSkillsGit, setUserSkillsInbound, skills } = getCtx();
    const trimmed = remoteUrl.trim();
    if (!trimmed) {
      throw new Error('Enter a Git remote URL.');
    }
    const generation = authoritativeGenerationRef.current + 1;
    authoritativeGenerationRef.current = generation;
    setStatus('ready');

    if (!window.__TAURI_INTERNALS__) {
      const normalized = normalizeUserSkillsGitStatus({
        repo_path: previewPaths.userSkillsRoot,
        remote_url: trimmed,
        branch: 'main',
        state: 'clean',
        dirty: false
      });
      if (generation !== authoritativeGenerationRef.current) {
        return null;
      }
      setUserSkillsGit(normalized);
      setUserSkillsInbound(null);
      setNotice('User skills remote saved.');
      return normalized;
    }

    const result = await invoke('set_user_skills_git_remote', {
      request: { remote_url: trimmed }
    });
    const normalized = normalizeUserSkillsGitStatus(result);
    if (generation !== authoritativeGenerationRef.current) {
      return null;
    }
    setUserSkillsGit(normalized);
    setUserSkillsInbound(null);
    setNotice('User skills remote saved.');
    return normalized;
  }

  async function scanWorkspaceSkills(workspace) {
    const { setError, setImportReview, setNotice, setStatus, setWorkspaces, skills } = getCtx();
    const reviewMeta = workspaceSkillReviewMeta(workspace);

    setStatus('scanning_workspace_skills');
    setError('');
    setNotice('');

    if (!window.__TAURI_INTERNALS__) {
      const previewCandidates = applyPreviewImportStatuses(
        previewCandidatesForWorkspace(workspace).map(normalizeImportCandidate),
        skills
      );
      const candidates = normalizeImportCandidateGroups([], previewCandidates);

      setImportReview({
        open: true,
        candidates,
        collections: [],
        errors: [],
        ...reviewMeta
      });
      setNotice(`Browser preview is using mock skills for ${workspace.displayName}.`);
      setStatus('prototype');
      return;
    }

    try {
      const scan = await invoke('scan_workspace_import_candidates', { path: workspace.path });
      const workspaceRows = await invoke('list_workspaces').catch(() => []);
      const candidates = normalizeImportCandidateGroups(scan.groups || [], scan.candidates || []);
      const collections = normalizeImportCollections(scan.collections || []);

      setWorkspaces(normalizeWorkspaces(workspaceRows));
      setImportReview({
        open: true,
        candidates,
        collections,
        errors: scan.errors || [],
        ...reviewMeta
      });
      setNotice(candidates.length === 0 ? `${workspace.displayName}: no skills found.` : '');
      setStatus('ready');
    } catch (workspaceError) {
      setError(workspaceError.message || String(workspaceError) || 'Unable to scan workspace skills.');
      setStatus('ready');
    }
  }

  async function chooseWorkspaceDialogFolder() {
    const { error, previewWorkspaceDialog, setStatus, setWorkspaceDialog, status, workspaceDialog, workspacePreviewRequestRef } = getCtx();
    if (!window.__TAURI_INTERNALS__ || autoRefreshBlockedStatuses.has(status)) return;

    const kind = workspaceDialog.kind;
    setStatus('choosing_workspace');
    try {
      const selectedPath = await chooseWorkspaceDirectory(openDialog);
      if (selectedPath === null) {
        setStatus('ready');
        return;
      }
      workspacePreviewRequestRef.current += 1;
      setWorkspaceDialog((current) => ({
        ...current,
        path: selectedPath,
        error: '',
        preview: null,
        selectedRoot: ''
      }));
      await previewWorkspaceDialog(kind, selectedPath);
    } catch (pickerError) {
      setWorkspaceDialog((current) => ({
        ...current,
        error: `Unable to choose a local folder. ${pickerError.message || String(pickerError)}`
      }));
      setStatus('ready');
    }
  }

  async function submitWorkspaceDialog(event) {
    const { error, filter, refreshDeployDialogRows, setError, setNotice, setStatus, setWorkspaceDialog, setWorkspaces, skills, workspaceDialog, workspaces } = getCtx();
    event.preventDefault();
    const workspacePath = workspaceDialog.path.trim();
    const preview = workspaceDialog.preview;
    const selectedRoot = preview?.roots.find((root) => root.path === workspaceDialog.selectedRoot);

    if (!workspacePath || !preview || !selectedRoot) {
      setWorkspaceDialog((current) => ({
        ...current,
        error: 'Preview the project or skills folder before continuing.'
      }));
      return;
    }

    setStatus('setting_up_workspace');
    setError('');
    setNotice('');

    if (!window.__TAURI_INTERNALS__) {
      const workspace = normalizeWorkspace({
        canonical_path: selectedRoot.path,
        path: selectedRoot.path,
        kind: workspaceDialog.kind,
        source: 'manual',
        agent_id: selectedRoot.agentId,
        profile_id: selectedRoot.profileId,
        profile_name: selectedRoot.profileName,
        root_key: selectedRoot.rootKey,
        format: selectedRoot.format,
        skill_count: 0,
        last_scan_error_count: 0,
        last_scanned_at: new Date().toISOString()
      });
      setWorkspaces((current) =>
        [...current.filter((item) => item.canonicalPath !== workspace.canonicalPath), workspace]
          .sort((left, right) => left.path.localeCompare(right.path))
      );
      refreshDeployDialogRows(
        [...workspaces.filter((item) => item.canonicalPath !== workspace.canonicalPath), workspace]
          .sort((left, right) => left.path.localeCompare(right.path))
      );
      setWorkspaceDialog({
        open: false,
        path: '',
        kind: 'user',
        error: '',
        preview: null,
        selectedRoot: ''
      });
      setNotice(selectedRoot.exists ? 'Workspace added.' : `Created and added ${selectedRoot.relativePath}.`);
      setStatus('prototype');
      return;
    }

    try {
      const result = await invoke('apply_workspace_setup', {
        request: {
          selected_path: workspacePath,
          kind: workspaceDialog.kind,
          selected_root: selectedRoot.path,
          create_missing: !selectedRoot.exists,
          preview_id: preview.previewId
        }
      });
      const workspace = result.workspace;
      const rows = await invoke('list_workspaces').catch(() => [workspace]);
      const normalizedRows = normalizeWorkspaces(rows);
      setWorkspaces(normalizedRows);
      refreshDeployDialogRows(normalizedRows);
      setWorkspaceDialog({
        open: false,
        path: '',
        kind: 'user',
        error: '',
        preview: null,
        selectedRoot: ''
      });
      setNotice(
        result.created_path
          ? `Created and added: ${normalizeWorkspace(workspace).compactPath}`
          : `Workspace added: ${normalizeWorkspace(workspace).compactPath}`
      );
      setStatus('ready');
    } catch (workspaceError) {
      setWorkspaceDialog((current) => ({
        ...current,
        error: workspaceError.message || String(workspaceError) || 'Unable to add workspace.'
      }));
      setStatus('ready');
    }
  }

  return {
    navigateToPage,
    openDashboard,
    clearDashboardFilters,
    openHistory,
    loadHistory,
    openRankings,
    cancelUsageRankingRequest,
    loadUsageRankings,
    openRankedSkill,
    importRankedSkill,
    openGithubCollectionRollback,
    applyGithubCollectionRollback,
    openImportRevertDialog,
    confirmImportRevert,
    openSkillTypeChangeDialog,
    closeSkillTypeChangeDialog,
    confirmSkillTypeChange,
    updateDeployWarningConfirmation,
    updateDeployUndeployConfirmation,
    refreshDeployDialogRows,
    submitDeployDialog,
    loadUserSkillContext,
    searchRemoteSourceCandidates,
    viewRemoteSourceCandidate,
    openRemoteSourceUrl,
    openLocalSkillFolder,
    closeRemoteSourceCandidateBind,
    confirmRemoteSourceCandidateBind,
    closeRemoteVersionDialog,
    activateRemoteVersionPath,
    applyRemoteVersionChange,
    activateRemoteInstallPath,
    applyRemoteInstall,
    saveUserSkillsGitRemote,
    scanWorkspaceSkills,
    chooseWorkspaceDialogFolder,
    submitWorkspaceDialog
  };
}
