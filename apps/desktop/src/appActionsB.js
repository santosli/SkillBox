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
  async function checkAppUpdate({ automatic = false } = {}) {
    const { appUpdate, setAppUpdate, setError, setNotice } = getCtx();
    if (!automatic) {
      setNotice('');
    }

    if (!window.__TAURI_INTERNALS__) {
      const disabledStatus = normalizeAppUpdateStatus(
        {
          disabled: true,
          current_version: desktopPackage.version,
          message: 'App updater is disabled in browser preview.'
        },
        desktopPackage.version
      );
      setAppUpdate(disabledStatus);
      if (!automatic) {
        setNotice(disabledStatus.message);
      }
      return;
    }

    setAppUpdate((current) => ({
      ...current,
      state: 'checking',
      message: ''
    }));

    try {
      const result = await invoke('check_app_update', {
        force: !automatic
      });
      const nextStatus = normalizeAppUpdateStatus(result, desktopPackage.version);
      setAppUpdate(nextStatus);

      if (nextStatus.available && (!automatic || !appUpdate.available)) {
        setNotice(appUpdateNotice(nextStatus));
      } else if (!automatic) {
        setNotice(appUpdateNotice(nextStatus) || nextStatus.message || 'SkillBox is up to date.');
      }
    } catch (updateError) {
      const message =
        updateError.message || String(updateError) || 'Unable to check for app updates.';
      setAppUpdate((current) =>
        appUpdateStatusAfterCheckError(
          current,
          message,
          desktopPackage.version,
          new Date().toISOString()
        )
      );
      if (!automatic) {
        setError(message);
      }
    }
  }

  async function runHealthCheck() {
    const { setDoctorReport, setError, setStatus } = getCtx();
    setStatus('checking_health');
    setError('');

    if (!window.__TAURI_INTERNALS__) {
      setDoctorReport(
        normalizeDoctorReport({
          checked_at: new Date().toISOString(),
          schema_version: 3,
          latest_schema_version: 3,
          healthy: true,
          repair_preview: true,
          issues: []
        })
      );
      setStatus('prototype');
      return;
    }

    try {
      const report = await invoke('run_doctor', {
        request: { repair_preview: true }
      });
      setDoctorReport(normalizeDoctorReport(report));
      setStatus('ready');
    } catch (doctorError) {
      setError(doctorError.message || String(doctorError));
      setStatus('ready');
    }
  }

  async function repairStaleDeploymentRecords() {
    const { runHealthCheck, setError, setNotice, setStatus } = getCtx();
    setStatus('repairing_stale_deployments');
    setError('');

    try {
      const result = window.__TAURI_INTERNALS__
        ? await invoke('repair_stale_deployment_records')
        : { removed_deployment_records: 1 };
      const { removedDeploymentRecords } = normalizeStaleDeploymentRepairResult(result);
      const recordLabel = removedDeploymentRecords === 1 ? 'record' : 'records';
      setNotice(
        `Cleaned ${removedDeploymentRecords} stale SQLite deployment ${recordLabel}. No runtime files were deleted.`
      );
      await runHealthCheck();
    } catch (repairError) {
      setError(repairError.message || String(repairError));
      setStatus(window.__TAURI_INTERNALS__ ? 'ready' : 'prototype');
    }
  }

  function requestAppUpdateInstall() {
    const { appUpdate, error, setAppUpdateDialog, setError, setNotice } = getCtx();
    if (appUpdateInstallBlocked) {
      setNotice('Finish the current SkillBox operation before installing an app update.');
      return;
    }

    if (!appUpdate.available || appUpdate.state === 'checking' || appUpdate.state === 'installing') {
      return;
    }

    setError('');
    setNotice('');
    setAppUpdateDialog({ open: true, error: '' });
  }

  function closeAppUpdateDialog() {
    const { appUpdate, error, setAppUpdateDialog } = getCtx();
    if (appUpdate.state === 'installing') {
      return;
    }

    setAppUpdateDialog({ open: false, error: '' });
  }

  async function installAppUpdate() {
    const { appUpdateDialog, error, setAppUpdate, setAppUpdateDialog, setError, setNotice } = getCtx();
    if (!appUpdateDialog.open) {
      return;
    }

    if (appUpdateInstallBlocked) {
      setAppUpdateDialog({ open: false, error: '' });
      setNotice('Finish the current SkillBox operation before installing an app update.');
      return;
    }

    if (!window.__TAURI_INTERNALS__) {
      setAppUpdateDialog({ open: false, error: '' });
      setNotice('Development preview only. Packaged release builds perform the signed update.');
      return;
    }

    setAppUpdate((current) => ({
      ...current,
      state: 'installing',
      message: ''
    }));
    setAppUpdateDialog((current) => ({ ...current, error: '' }));

    try {
      const checked = normalizeAppUpdateStatus(
        await invoke('check_app_update', { force: true }),
        desktopPackage.version
      );
      if (!checked.available) {
        setAppUpdate(checked);
        setAppUpdateDialog({ open: false, error: '' });
        setNotice(appUpdateNotice(checked) || 'SkillBox is already up to date.');
        return;
      }
      setAppUpdate({
        ...checked,
        state: 'installing'
      });
      await invoke('install_app_update');
      setAppUpdateDialog({ open: false, error: '' });
      setNotice('App update installed. Restarting SkillBox.');
    } catch (updateError) {
      const message =
        updateError.message || String(updateError) || 'Unable to install the app update.';
      setAppUpdate((current) => ({
        ...current,
        state: current.available ? 'available' : 'error',
        message
      }));
      setAppUpdateDialog((current) => ({ ...current, error: message }));
      setError(message);
    }
  }

  async function scanForImportCandidates() {
    const { error, importScanControllerRef, importScanTimingRef, setError, setImportReview, setNotice, setStatus, setWorkspaces, skills } = getCtx();
    if (!importScanControllerRef.current) {
      importScanControllerRef.current = createImportScanRequestController();
    }
    const scanController = importScanControllerRef.current;
    const scanId = scanController.begin();
    if (scanId == null) {
      return;
    }

    importScanTimingRef.current = {
      startedAt: performance.now(),
      shellPaintedAt: null,
      commandStartedAt: null,
      commandFinishedAt: null
    };
    setImportReview((current) => ({
      ...current,
      open: true,
      loading: true,
      candidates: [],
      collections: [],
      errors: [],
      scanError: '',
      scanProgress: {
        phase: 'preparing',
        processed: 0,
        total: null,
        uniqueRepositories: 0
      },
      diagnostics: null
    }));
    setStatus('scanning');
    setError('');
    setNotice('');

    try {
      await waitForNextPaint();
      if (!scanController.isCurrent(scanId)) {
        return;
      }
      if (importScanTimingRef.current) {
        importScanTimingRef.current.shellPaintedAt = performance.now();
      }

      if (!window.__TAURI_INTERNALS__) {
        const previewOptions = browserImportScanOptions(window.location.search);
        setImportReview((current) => ({
          ...current,
          scanProgress: {
            phase: 'validating candidates',
            processed: 0,
            total: previewImportCandidateGroups.length,
            uniqueRepositories: previewImportCollections.length
          }
        }));
        await waitForImportScanDelay(previewOptions.delayMs);
        if (!scanController.isCurrent(scanId)) {
          return;
        }
        if (previewOptions.error) {
          throw new Error('Browser preview scan failed. Retry the local scan.');
        }
        setWorkspaces(normalizeWorkspaces(previewWorkspaces));
        setImportReview({
          open: true,
          loading: false,
          candidates: normalizeImportCandidateGroups(previewImportCandidateGroups),
          collections: normalizeImportCollections(previewImportCollections),
          errors: [],
          scanError: '',
          scanProgress: null,
          diagnostics: {
            candidateCount: previewImportCandidateGroups.length,
            uniqueRepositoryCount: previewImportCollections.length,
            repositoryInspections: previewImportCollections.length,
            repositoryCacheHits: 0,
            snapshotHashComputations: 0,
            snapshotCacheHits: 0,
            elapsedMs: Math.round(performance.now() - importScanTimingRef.current.startedAt)
          },
          title: 'Import Review',
          subtitle: 'Confirm each skill type before SkillBox copies it into the managed store.',
          noticePrefix: ''
        });
        if (import.meta.env.DEV) {
          const timing = importScanTimingRef.current;
          console.info('[SkillBox] import scan diagnostics', {
            shellPaintMs: timing?.shellPaintedAt == null ? null : Math.round(timing.shellPaintedAt - timing.startedAt),
            commandMs: null,
            totalMs: timing ? Math.round(performance.now() - timing.startedAt) : null,
            diagnostics: {
              candidateCount: previewImportCandidateGroups.length,
              uniqueRepositoryCount: previewImportCollections.length
            }
          });
        }
        setNotice('Browser preview is using mock scan candidates.');
        setStatus('prototype');
        scanController.finish(scanId);
        return;
      }

      if (importScanTimingRef.current) {
        importScanTimingRef.current.commandStartedAt = performance.now();
      }
      const scan = await invoke('scan_import_candidates', importScanCommandArgs(scanId));
      if (importScanTimingRef.current) {
        importScanTimingRef.current.commandFinishedAt = performance.now();
      }
      if (!scanController.isCurrent(scanId)) {
        return;
      }
      const workspaceRows = await invoke('list_workspaces').catch(() => []);
      if (!scanController.isCurrent(scanId)) {
        return;
      }
      const candidates = normalizeImportCandidateGroups(scan.groups || [], scan.candidates || []);
      const collections = normalizeImportCollections(scan.collections || []);
      setWorkspaces(normalizeWorkspaces(workspaceRows));

      setImportReview({
        open: candidates.length > 0,
        loading: false,
        candidates,
        collections,
        errors: scan.errors || [],
        scanError: '',
        scanProgress: null,
        diagnostics: scan.diagnostics || null,
        title: 'Import Review',
        subtitle: 'Confirm each skill type before SkillBox copies it into the managed store.',
        noticePrefix: ''
      });
      if (import.meta.env.DEV) {
        const timing = importScanTimingRef.current;
        console.info('[SkillBox] import scan diagnostics', {
          shellPaintMs: timing?.shellPaintedAt == null ? null : Math.round(timing.shellPaintedAt - timing.startedAt),
          commandMs: timing?.commandStartedAt == null || timing?.commandFinishedAt == null
            ? null
            : Math.round(timing.commandFinishedAt - timing.commandStartedAt),
          totalMs: timing ? Math.round(performance.now() - timing.startedAt) : null,
          diagnostics: scan.diagnostics || null
        });
      }
      setNotice(candidates.length === 0 ? 'No new local skills found.' : '');
      setStatus('ready');
      scanController.finish(scanId);
    } catch (scanError) {
      if (!scanController.isCurrent(scanId)) {
        return;
      }
      const message = scanError.message || String(scanError) || 'Unable to scan local skill folders.';
      setImportReview((current) => ({
        ...current,
        open: true,
        loading: false,
        scanError: message,
        scanProgress: null
      }));
      setError('');
      setStatus('ready');
      scanController.finish(scanId);
    }
  }

  function closeImportReview() {
    const { importScanControllerRef, remoteImportRequestControllerRef, setImportReview, setStatus } = getCtx();
    importScanControllerRef.current?.invalidate();
    remoteImportRequestControllerRef.current?.invalidate();
    setImportReview((current) => ({
      ...current,
      open: false,
      loading: false,
      scanError: '',
      scanProgress: null
    }));
    setStatus((current) => current === 'scanning' ? 'ready' : current);
  }

  function updateImportCandidateGroup(groupId, updater) {
    const { setImportReview } = getCtx();
    setImportReview((current) => ({
      ...current,
      candidates: updater(current.candidates, groupId)
    }));
  }

  function toggleAllImportCandidates() {
    const { setImportReview } = getCtx();
    setImportReview((current) => ({
      ...current,
      candidates: toggleImportReviewSelection(current.candidates, current.collections)
    }));
  }

  async function importSelectedCandidates() {
    const { importReview, runCandidateImport, setLocalImportConfirmation, setNotice } = getCtx();
    const selected = selectedImportCandidates(
      importReview.candidates,
      importReview.collections
    );
    const collectionRequests = selectedImportCollectionRequests(
      importReview.candidates,
      importReview.collections
    );
    if (selected.length === 0 && collectionRequests.length === 0) {
      setNotice('Select at least one candidate without conflicts to import.');
      return;
    }

    if (shouldConfirmLocalImport(selected)) {
      setLocalImportConfirmation({
        open: true,
        candidates: selected,
        collectionRequests,
        noticePrefix: importReview.noticePrefix || ''
      });
      return;
    }

    await runCandidateImport(selected, importReview.noticePrefix || '', collectionRequests);
  }

  async function runCandidateImport(selected, noticePrefix = '', collectionRequests = []) {
    const { filter, importReview, loadUsageRankings, page, refresh, remoteImportRequestControllerRef, setError, setImportReview, setIsFirstUse, setNotice, setSelectedName, setSkills, setStatus, skills, usageRankingFilters } = getCtx();
    const remoteRequestId = collectionRequests.length > 0
      ? importReview.remoteRequestId
      : null;
    const isCurrentRemoteRequest = () =>
      remoteRequestId == null
      || remoteImportRequestControllerRef.current?.isCurrent(remoteRequestId) === true;
    setStatus('importing');
    setError('');
    setNotice('');

    if (!window.__TAURI_INTERNALS__) {
      const importedSkills = selected.map(candidateToPreviewSkill);

      setSkills((current) => mergeSkills(current, importedSkills));
      setSelectedName('');
      setIsFirstUse(false);
      setImportReview({ open: false, candidates: [], collections: [], errors: [], noticePrefix: '' });
      setStatus('prototype');
      setNotice(importNotice(noticePrefix, `Mock imported ${importedSkills.length} skills.`));
      return;
    }

    try {
      const result = selected.length > 0
        ? await invoke('import_candidates', { items: importRequestItems(selected) })
        : { imported: [], errors: [] };
      const collectionResults = [];
      for (const request of collectionRequests) {
        const command = request.sourceKind === 'github_remote'
          ? (importReview.reviewMode === 'update'
            ? 'apply_github_skill_collection_update'
            : 'apply_github_skill_collection')
          : 'apply_import_collection';
        const requestBody = {
          collection_id: request.collectionId,
          preview_id: request.previewId,
          selections: request.selections.map((selection) => ({
            relative_path: selection.relativePath,
            group_id: selection.groupId,
            variant_id: selection.variantId,
            skill_type: selection.skillType
          })),
          actor: 'desktop'
        };
        if (request.sourceKind === 'github_remote') {
          requestBody.source_url = request.sourceUrl;
        } else {
          requestBody.worktree_root = request.worktreeRoot;
        }
        collectionResults.push(await invoke(command, { request: requestBody }));
        if (!isCurrentRemoteRequest()) {
          return;
        }
      }

      if (!isCurrentRemoteRequest()) {
        return;
      }
      setImportReview({ open: false, candidates: [], collections: [], errors: [], noticePrefix: '' });
      await refresh();
      if (!isCurrentRemoteRequest()) {
        return;
      }
      if (page === 'rankings') {
        await loadUsageRankings(usageRankingFilters);
        if (!isCurrentRemoteRequest()) {
          return;
        }
      }
      const collectionCount = collectionResults.reduce(
        (count, collection) => count + (collection.imported || []).length,
        0
      );
      const summary = [
        selected.length > 0 ? importBatchNotice(result) : '',
        collectionCount > 0 ? `Imported ${collectionCount} collection skill${collectionCount === 1 ? '' : 's'}.` : ''
      ].filter(Boolean).join(' ');
      setNotice(importNotice(noticePrefix, summary || 'Import completed.'));
    } catch (importError) {
      if (!isCurrentRemoteRequest()) {
        return;
      }
      setError(importError.message || 'Unable to import selected skills.');
      setStatus('ready');
    }
  }

  function closeLocalImportConfirmation() {
    const { setLocalImportConfirmation, status } = getCtx();
    if (status === 'importing') {
      return;
    }
    setLocalImportConfirmation({ open: false, candidates: [], collectionRequests: [], noticePrefix: '' });
  }

  async function confirmLocalImport() {
    const { localImportConfirmation, runCandidateImport, setLocalImportConfirmation } = getCtx();
    const selected = localImportConfirmation.candidates;
    const collectionRequests = localImportConfirmation.collectionRequests || [];
    const noticePrefix = localImportConfirmation.noticePrefix || '';

    setLocalImportConfirmation({ open: false, candidates: [], collectionRequests: [], noticePrefix: '' });
    await runCandidateImport(selected, noticePrefix, collectionRequests);
  }

  async function saveStatusRefreshIntervalMinutes(minutes) {
    const { preferences, refresh, setPreferences } = getCtx();
    const intervalMinutes = Number(minutes);

    if (!Number.isInteger(intervalMinutes) || intervalMinutes < 1 || intervalMinutes > 1440) {
      throw new Error('Auto refresh interval must be between 1 and 1440 minutes.');
    }

    if (!window.__TAURI_INTERNALS__) {
      try {
        window.localStorage.setItem(
          previewStatusRefreshIntervalStorageKey,
          String(intervalMinutes)
        );
      } catch {
        // Browser preview can run without durable storage; keep the session preference in React state.
      }
      const nextPreferences = {
        ...preferences,
        statusRefreshIntervalMinutes: intervalMinutes
      };
      setPreferences(nextPreferences);
      return nextPreferences;
    }

    const storedPreferences = await invoke('set_status_refresh_interval_minutes', {
      minutes: intervalMinutes
    });
    const nextPreferences = normalizePreferences(storedPreferences);
    setPreferences(nextPreferences);
    return nextPreferences;
  }

  async function saveRemoteUpdateTimeoutSeconds(seconds) {
    const { preferences, setPreferences } = getCtx();
    const timeoutSeconds = Number(seconds);

    if (!Number.isInteger(timeoutSeconds) || timeoutSeconds < 5 || timeoutSeconds > 300) {
      throw new Error('Git check timeout must be between 5 and 300 seconds.');
    }

    if (!window.__TAURI_INTERNALS__) {
      try {
        window.localStorage.setItem(
          previewRemoteUpdateTimeoutStorageKey,
          String(timeoutSeconds)
        );
      } catch {
        // Browser preview can run without durable storage; keep the session preference in React state.
      }
      const nextPreferences = {
        ...preferences,
        remoteUpdateTimeoutSeconds: timeoutSeconds
      };
      setPreferences(nextPreferences);
      return nextPreferences;
    }

    const storedPreferences = await invoke('set_remote_update_timeout_seconds', {
      seconds: timeoutSeconds
    });
    const nextPreferences = normalizePreferences(storedPreferences);
    setPreferences(nextPreferences);
    return nextPreferences;
  }

  async function saveCommitSummaryCli(cliPath) {
    const { preferences, setPreferences } = getCtx();
    const trimmed = String(cliPath || '').trim();

    if (!window.__TAURI_INTERNALS__) {
      try {
        window.localStorage.setItem(previewCommitSummaryCliStorageKey, trimmed);
      } catch {
        // Browser preview can run without durable storage; keep the session preference in React state.
      }
      const nextPreferences = {
        ...preferences,
        commitSummaryCli: trimmed,
        resolvedCommitSummaryCli: trimmed
      };
      setPreferences(nextPreferences);
      return nextPreferences;
    }

    const storedPreferences = await invoke('set_commit_summary_cli', {
      cliPath: trimmed
    });
    const nextPreferences = normalizePreferences(storedPreferences);
    setPreferences(nextPreferences);
    return nextPreferences;
  }

  async function installUsageHook(target) {
    const { refreshUsageHookStatuses, setError, setNotice, setStatus, setUsageHooks } = getCtx();
    setStatus('installing_usage_hook');
    setError('');
    setNotice('');

    if (!window.__TAURI_INTERNALS__) {
      setUsageHooks((current) => {
        const normalized = normalizeUsageHookStatuses(current);
        const selected = normalized.find((hook) => hook.target === target);
        const sharedConfigKey = selected?.sharedConfigKey || target;
        return normalized.map((hook) =>
          hook.sharedConfigKey === sharedConfigKey
            ? { ...hook, installed: true }
            : hook
        );
      });
      setNotice('Usage hook injection is enabled in preview.');
      setStatus('ready');
      return;
    }

    try {
      await invoke('install_usage_hook', { target });
      await refreshUsageHookStatuses({ silent: true });
      setNotice('Usage hook injection updated.');
      setStatus('ready');
    } catch (hookError) {
      setError(hookError.message || String(hookError) || 'Unable to install usage hook.');
      setStatus('ready');
    }
  }

  async function openSyncDialog() {
    const { error, setError, setNotice, setStatus, setSyncDialog, skills, userSkillsGit } = getCtx();
    setError('');
    setNotice('');
    setSyncDialog({
      open: true,
      loading: true,
      remoteUrl: userSkillsGit.remoteUrl || '',
      commitMessage: defaultSyncCommitMessage,
      commitMessageEdited: false,
      push: true,
      error: '',
      syncLog: [],
      generating: false,
      generateSource: '',
      changes: normalizeUserSkillsGitChanges(null),
      selectedPaths: [],
      activePath: ''
    });

    if (!window.__TAURI_INTERNALS__) {
      const changes = normalizeUserSkillsGitChanges(previewUserSkillsGitChanges());
      setSyncDialog((current) => ({
        ...current,
        loading: false,
        changes,
        selectedPaths: changes.selectedPaths,
        activePath: changes.activePath,
        commitMessage: suggestUserSkillsCommitMessage(changes.files, changes.selectedPaths)
      }));
      return;
    }

    setStatus('preparing_sync');
    try {
      const result = await invoke('user_skills_git_changes');
      const changes = normalizeUserSkillsGitChanges(result);
      setSyncDialog((current) => ({
        ...current,
        loading: false,
        remoteUrl: current.remoteUrl || changes.remoteUrl || '',
        changes,
        selectedPaths: changes.selectedPaths,
        activePath: changes.activePath,
        commitMessage: current.commitMessageEdited
          ? current.commitMessage
          : suggestUserSkillsCommitMessage(changes.files, changes.selectedPaths)
      }));
      setStatus('ready');
    } catch (syncError) {
      setSyncDialog((current) => ({
        ...current,
        loading: false,
        error: syncError.message || String(syncError) || 'Unable to load user skills changes.'
      }));
      setStatus('ready');
    }
  }

  function activateSyncDialogPath(path) {
    const { setSyncDialog } = getCtx();
    setSyncDialog((current) => ({ ...current, activePath: path }));
  }

  async function generateSyncDialogMessage() {
    const { error, setSyncDialog, syncDialog } = getCtx();
    const selectedPaths = syncDialog.selectedPaths;
    const files = syncDialog.changes.files;
    setSyncDialog((current) => ({
      ...current,
      generating: true,
      generateSource: '',
      error: ''
    }));

    if (!window.__TAURI_INTERNALS__) {
      setSyncDialog((current) => ({
        ...current,
        generating: false,
        generateSource: 'heuristic',
        commitMessage: suggestUserSkillsCommitMessage(files, selectedPaths),
        commitMessageEdited: false,
        error: ''
      }));
      return;
    }

    try {
      const result = normalizeSuggestedUserSkillsCommit(
        await invoke('suggest_user_skills_commit_message', {
          request: { selected_paths: selectedPaths }
        })
      );
      setSyncDialog((current) => {
        if (!current.open) return current;
        return {
          ...current,
          generating: false,
          generateSource: result.source || (result.message ? 'cli' : 'heuristic'),
          commitMessage: result.message || suggestUserSkillsCommitMessage(files, selectedPaths),
          commitMessageEdited: false,
          error: ''
        };
      });
    } catch (generateError) {
      setSyncDialog((current) => {
        if (!current.open) return current;
        return {
          ...current,
          generating: false,
          generateSource: '',
          error:
            generateError.message ||
            generateError.error ||
            String(generateError) ||
            'Unable to generate commit message.'
        };
      });
    }
  }

  async function checkUserSkillsInbound() {
    const { authoritativeGenerationRef, setError, setNotice, setStatus, setUserSkillsInbound, skills } = getCtx();
    const generation = authoritativeGenerationRef.current + 1;
    authoritativeGenerationRef.current = generation;
    setStatus('checking_inbound');
    setError('');
    setNotice('');

    if (!window.__TAURI_INTERNALS__) {
      const inboundPreviewMode = new URLSearchParams(window.location.search).get('inbound') || 'behind';
      const checked = normalizeUserSkillsInboundStatus(
        previewUserSkillsInboundStatus(inboundPreviewMode)
      );
      if (generation !== authoritativeGenerationRef.current) {
        return null;
      }
      setUserSkillsInbound(checked);
      setNotice(checked.message);
      setStatus('prototype');
      return checked;
    }

    try {
      const result = await invoke('check_user_skills_inbound');
      const checked = normalizeUserSkillsInboundStatus(result);
      if (generation !== authoritativeGenerationRef.current) {
        return null;
      }
      setUserSkillsInbound(checked);
      setNotice(checked.fetchError || checked.message);
      setStatus('ready');
      return checked;
    } catch (checkError) {
      if (generation !== authoritativeGenerationRef.current) {
        return null;
      }
      const message =
        checkError.message || String(checkError) || 'Unable to check incoming user skills.';
      setUserSkillsInbound((current) => ({
        ...normalizeUserSkillsInboundStatus(current),
        relation: 'unknown',
        fetchError: message,
        message
      }));
      setNotice(message);
      setStatus('ready');
      return null;
    }
  }

  async function openUserSkillsInboundReview() {
    const { authoritativeGenerationRef, error, inboundReviewRequestControllerRef, setInboundReviewDialog, setStatus, setUserSkillsInbound, skills, status } = getCtx();
    const generation = authoritativeGenerationRef.current + 1;
    authoritativeGenerationRef.current = generation;
    const browserPreview = !window.__TAURI_INTERNALS__;
    setInboundReviewDialog({
      open: true,
      loading: true,
      applying: false,
      preview: null,
      activePath: '',
      error: ''
    });
    setStatus('previewing_inbound');

    await inboundReviewRequestControllerRef.current.run({
      loadPreview: async () => {
        if (browserPreview) {
          const inboundPreviewMode =
            new URLSearchParams(window.location.search).get('inbound') || 'behind';
          return normalizeUserSkillsInboundPreview(
            previewUserSkillsInbound(inboundPreviewMode)
          );
        }
        return normalizeUserSkillsInboundPreview(await invoke('preview_user_skills_inbound'));
      },
      onSuccess: (preview) => {
        if (generation !== authoritativeGenerationRef.current) {
          return;
        }
        setUserSkillsInbound(preview.status);
        setInboundReviewDialog({
          open: true,
          loading: false,
          applying: false,
          preview,
          activePath: preview.files[0]?.path || '',
          error: ''
        });
        setStatus(browserPreview ? 'prototype' : 'ready');
      },
      onError: (previewError) => {
        if (generation !== authoritativeGenerationRef.current) {
          return;
        }
        setInboundReviewDialog((current) => ({
          ...current,
          loading: false,
          error:
            previewError.message ||
            String(previewError) ||
            'Unable to preview incoming user skills.'
        }));
        setStatus(browserPreview ? 'prototype' : 'ready');
      }
    });
  }

  async function openUserSkillsRepository() {
    const { error, inboundReviewDialog, setInboundReviewDialog, setNotice, skills, status, userSkillsGit, userSkillsInbound } = getCtx();
    const repoPath =
      inboundReviewDialog.preview?.status.repoPath ||
      userSkillsInbound?.repoPath ||
      userSkillsGit.repoPath;
    if (!repoPath) return;

    if (window.__TAURI_INTERNALS__) {
      try {
        await invoke('open_local_path', { path: repoPath });
        return;
      } catch (openError) {
        setInboundReviewDialog((current) => ({
          ...current,
          error: openError.message || String(openError)
        }));
        return;
      }
    }
    setNotice(`User skills repository: ${compactPath(repoPath)}`);
  }

  async function copyUserSkillsRepositoryPath() {
    const { error, inboundReviewDialog, setInboundReviewDialog, setNotice, skills, status, userSkillsGit, userSkillsInbound } = getCtx();
    const repoPath =
      inboundReviewDialog.preview?.status.repoPath ||
      userSkillsInbound?.repoPath ||
      userSkillsGit.repoPath;
    if (!repoPath) return;

    try {
      await navigator.clipboard.writeText(repoPath);
      setNotice('Copied the user skills repository path.');
    } catch (copyError) {
      setInboundReviewDialog((current) => ({
        ...current,
        error: copyError.message || String(copyError) || 'Unable to copy repository path.'
      }));
    }
  }

  return {
    checkAppUpdate,
    runHealthCheck,
    repairStaleDeploymentRecords,
    requestAppUpdateInstall,
    closeAppUpdateDialog,
    installAppUpdate,
    scanForImportCandidates,
    closeImportReview,
    updateImportCandidateGroup,
    toggleAllImportCandidates,
    importSelectedCandidates,
    runCandidateImport,
    closeLocalImportConfirmation,
    confirmLocalImport,
    saveStatusRefreshIntervalMinutes,
    saveRemoteUpdateTimeoutSeconds,
    saveCommitSummaryCli,
    installUsageHook,
    openSyncDialog,
    activateSyncDialogPath,
    generateSyncDialogMessage,
    checkUserSkillsInbound,
    openUserSkillsInboundReview,
    openUserSkillsRepository,
    copyUserSkillsRepositoryPath
  };
}
