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
  async function refresh() {
    const { authoritativeGenerationRef, favoriteNames, isFirstUse, paths, setDashboardTagOverrides, setError, setFavoriteNames, setIsFirstUse, setLastStatusCheckedAt, setNotice, setPaths, setPreferences, setRemoteSkillUpdates, setSelectedName, setSkillCollections, setSkills, setStatus, setUsageHooks, setUserSkillsGit, setWorkspaces, skills } = getCtx();
    const generation = authoritativeGenerationRef.current + 1;
    authoritativeGenerationRef.current = generation;
    setStatus('loading');
    setError('');

    try {
      if (!window.__TAURI_INTERNALS__) {
        throw new Error('Browser preview is mocking an empty managed store. Run inside Tauri to use the local skill bridge.');
      }

      const [
        state,
        storedPreferences,
        gitStatus,
        cachedRemoteUpdatesResult,
        workspaceRows,
        usageHookRows,
        storedSkillUserMetadata,
        storedCollections
      ] = await Promise.all([
        invoke('managed_state'),
        invoke('managed_preferences').catch(() => null),
        invoke('user_skills_git_status').catch(() => null),
        invoke('cached_remote_skill_updates').catch(() => null),
        invoke('list_workspaces').catch(() => []),
        invoke('usage_hook_statuses').catch(() => []),
        invoke('list_skill_user_metadata').catch(() => null),
        invoke('list_skill_collections').catch(() => [])
      ]);
      const managedSkills = state.skills?.map(normalizeSkill) || [];
      const cachedRemoteUpdates = normalizeRemoteSkillUpdates(cachedRemoteUpdatesResult);
      let resolvedSkillUserMetadata = storedSkillUserMetadata;
      const legacyMetadata = legacySkillUserMetadataUpdates(
        readDashboardFavorites(),
        readDashboardTagOverrides()
      );
      if (resolvedSkillUserMetadata && legacyMetadata.length > 0) {
        resolvedSkillUserMetadata = await invoke('migrate_legacy_skill_user_metadata', {
          items: legacyMetadata
        });
        clearLegacyDashboardMetadata();
      }
      const skillUserMetadataState = normalizeSkillUserMetadata(resolvedSkillUserMetadata || []);

      if (generation !== authoritativeGenerationRef.current) {
        return;
      }
      setSkills(managedSkills);
      setWorkspaces(normalizeWorkspaces(workspaceRows));
      setUsageHooks(normalizeUsageHookStatuses(usageHookRows));
      setPaths(normalizePaths(state.paths));
      setPreferences(normalizePreferences(storedPreferences));
      setUserSkillsGit(normalizeUserSkillsGitStatus(gitStatus));
      setRemoteSkillUpdates(cachedRemoteUpdates);
      setSkillCollections(Array.isArray(storedCollections) ? storedCollections : []);
      if (resolvedSkillUserMetadata) {
        setFavoriteNames(skillUserMetadataState.favoriteNames);
        setDashboardTagOverrides(skillUserMetadataState.tagOverrides);
      }
      setLastStatusCheckedAt(cachedRemoteUpdates.checkedAt || '');
      setIsFirstUse(Boolean(state.isFirstUse ?? state.is_first_use));
      setSelectedName((currentName) =>
        currentName && managedSkills.some((skill) => skill.name === currentName) ? currentName : ''
      );
      setStatus('ready');
    } catch (scanError) {
      if (generation !== authoritativeGenerationRef.current) {
        return;
      }
      setSkills(publicPreview ? previewSkills.map(normalizeSkill) : []);
      setWorkspaces(normalizeWorkspaces(previewWorkspaces));
      setPaths(previewPaths);
      setPreferences(readPreviewPreferences());
      setUserSkillsGit(normalizeUserSkillsGitStatus(null));
      setUsageHooks(normalizeUsageHookStatuses(null));
      setRemoteSkillUpdates(normalizeRemoteSkillUpdates(null));
      setSkillCollections([]);
      setLastStatusCheckedAt('');
      setIsFirstUse(!publicPreview);
      if (publicPreview) {
        setFavoriteNames(['release-helper', 'design-audit']);
        setDashboardTagOverrides({
          'release-helper': ['release'],
          'docs-reviewer': ['docs'],
          'design-audit': ['design', 'accessibility'],
          'research-digest': ['research'],
          'test-writer': ['testing'],
          'local-notes-sync': ['sync']
        });
      }
      setSelectedName('');
      setError('');
      setNotice(
        publicPreview
          ? ''
          : scanError.message || 'Browser preview is mocking an empty managed store.'
      );
      setStatus('prototype');
    }
  }

  async function refreshSkillStatuses({ automatic = false, skillName = '' } = {}) {
    const { authoritativeGenerationRef, filter, isFirstUse, paths, preferences, refresh, remoteSkillUpdates, setError, setIsFirstUse, setLastStatusCheckedAt, setNotice, setPaths, setRemoteSkillUpdates, setSelectedName, setSkills, setStatus, setUserSkillsGit, skills, status, userSkillsGit } = getCtx();
    const generation = authoritativeGenerationRef.current + 1;
    authoritativeGenerationRef.current = generation;
    setStatus('checking');
    setError('');
    if (!automatic) {
      setNotice('');
    }
    await waitForNextPaint();
    if (generation !== authoritativeGenerationRef.current) {
      return;
    }

    if (!window.__TAURI_INTERNALS__) {
      const nextRemoteUpdates = normalizeRemoteSkillUpdates({
        checked_at: new Date().toISOString(),
        statuses: skills
          .filter((skill) => skill.type === 'remote')
          .map((skill, index) => ({
            skill_name: skill.name,
            state: index === 0 ? 'update_available' : 'up_to_date',
            update_available: index === 0
          }))
      });

      if (generation !== authoritativeGenerationRef.current) {
        return;
      }
      setRemoteSkillUpdates(nextRemoteUpdates);
      setLastStatusCheckedAt(nextRemoteUpdates.checkedAt || new Date().toISOString());
      if (!automatic) {
        setNotice(dashboardStatusNotice({ userSkillsGit, remoteUpdates: nextRemoteUpdates }));
      }
      setStatus('prototype');
      return;
    }

    if (skillName) {
      try {
        const remoteUpdatesResult = await invoke('check_remote_skill_update', {
          skillName,
          timeoutSeconds: preferences.remoteUpdateTimeoutSeconds
        });
        const checkedRemoteUpdates = normalizeRemoteSkillUpdates(remoteUpdatesResult);
        const nextRemoteUpdates = mergeRemoteSkillUpdates(remoteSkillUpdates, checkedRemoteUpdates);

        if (generation !== authoritativeGenerationRef.current) {
          return;
        }
        setRemoteSkillUpdates(nextRemoteUpdates);
        setLastStatusCheckedAt(nextRemoteUpdates.checkedAt || new Date().toISOString());
        if (!automatic) {
          setNotice(dashboardStatusNotice({ userSkillsGit, remoteUpdates: nextRemoteUpdates }));
        }
        setStatus('ready');
        return;
      } catch (refreshError) {
        if (generation !== authoritativeGenerationRef.current) {
          return;
        }
        setLastStatusCheckedAt(new Date().toISOString());
        setError(refreshError.message || String(refreshError) || 'Unable to refresh skill status.');
        setStatus('ready');
        return;
      }
    }

    try {
      const [state, gitStatus, remoteUpdatesResult] = await Promise.all([
        invoke('managed_state'),
        invoke('user_skills_git_status').catch(() => null),
        invoke('check_remote_skill_updates', {
          timeoutSeconds: preferences.remoteUpdateTimeoutSeconds
        })
      ]);
      const managedSkills = state.skills?.map(normalizeSkill) || [];
      const nextUserSkillsGit = normalizeUserSkillsGitStatus(gitStatus);
      const nextRemoteUpdates = normalizeRemoteSkillUpdates(remoteUpdatesResult);

      if (generation !== authoritativeGenerationRef.current) {
        return;
      }
      setSkills(managedSkills);
      setPaths(normalizePaths(state.paths));
      setUserSkillsGit(nextUserSkillsGit);
      setRemoteSkillUpdates(nextRemoteUpdates);
      setLastStatusCheckedAt(nextRemoteUpdates.checkedAt || new Date().toISOString());
      setIsFirstUse(Boolean(state.isFirstUse ?? state.is_first_use));
      setSelectedName((currentName) =>
        currentName && managedSkills.some((skill) => skill.name === currentName) ? currentName : ''
      );
      if (!automatic) {
        setNotice(dashboardStatusNotice({ userSkillsGit: nextUserSkillsGit, remoteUpdates: nextRemoteUpdates }));
      }
      setStatus('ready');
    } catch (refreshError) {
      if (generation !== authoritativeGenerationRef.current) {
        return;
      }
      setLastStatusCheckedAt(new Date().toISOString());
      setError(refreshError.message || String(refreshError) || 'Unable to refresh skill status.');
      setStatus('ready');
    }
  }

  function openRemoteImport() {
    const { error, remoteImportRequestControllerRef, setError, setImportReview, setNotice, setRemoteImport } = getCtx();
    remoteImportRequestControllerRef.current?.invalidate();
    setError('');
    setNotice('');
    setImportReview((current) => ({ ...current, open: false }));
    setRemoteImport({
      open: true,
      mode: 'url',
      value: '',
      error: ''
    });
  }

  function closeRemoteImport() {
    const { error, remoteImportRequestControllerRef, setRemoteImport } = getCtx();
    remoteImportRequestControllerRef.current?.invalidate();
    setRemoteImport((current) => ({ ...current, open: false, error: '' }));
  }

  function updateRemoteImport(patch) {
    const { error, setRemoteImport } = getCtx();
    setRemoteImport((current) => ({ ...current, ...patch, error: '' }));
  }

  async function submitRemoteImport(event) {
    const { error, remoteImport, remoteImportRequestControllerRef, setImportReview, setNotice, setRemoteImport, setRemoteInstallDialog, setStatus, skills, status } = getCtx();
    event.preventDefault();
    const mode = remoteImport.mode;

    const value = remoteImport.value.trim();
    if (!value) {
      setRemoteImport((current) => ({ ...current, error: 'Enter a skill URL or Markdown file path.' }));
      return;
    }

    if (mode === 'url' && !isHttpUrl(value)) {
      setRemoteImport((current) => ({ ...current, error: 'Enter a full http(s) skill URL.' }));
      return;
    }

    if (mode === 'markdown' && !value.toLowerCase().endsWith('.md')) {
      setRemoteImport((current) => ({ ...current, error: 'Enter a local Markdown file path ending in .md.' }));
      return;
    }

    if (!remoteImportRequestControllerRef.current) {
      remoteImportRequestControllerRef.current = createRemoteImportRequestController();
    }
    const requestController = remoteImportRequestControllerRef.current;
    const requestId = requestController.begin();
    if (requestId == null) {
      return;
    }

    if (!window.__TAURI_INTERNALS__) {
      if (mode === 'url') {
        const preview = normalizeRemoteInstallPreview({
          preview_id: 'browser-preview',
          skill_name: remoteImportCandidate(remoteImport.mode, value).name || 'remote-skill',
          source_url: value,
          installed_sha: '1234567890abcdef',
          target_root: '/Users/demo/project/.agents/skills',
          compatibility: {
            preview_id: 'browser-compatibility-preview',
            profile_id: 'agents',
            profile_name: 'Agents',
            target_root: '/Users/demo/project/.agents/skills',
            status: 'warnings',
            issues: [
              {
                code: 'unknown_optional_frontmatter',
                severity: 'warning',
                message: 'Optional frontmatter fields are not declared by this runtime profile.',
                suggested_action: 'Review the fields before installing.'
              }
            ]
          },
          files: [
            {
              path: 'SKILL.md',
              status: 'A',
              diff: '@@\n+---\n+name: remote-skill\n+description: Preview skill\n+tools:\n+  - shell\n+---\n'
            }
          ]
        });
        setRemoteInstallDialog({
          open: true,
          loading: false,
          applying: false,
          preview,
          activePath: preview.activePath,
          confirmWarnings: false,
          title: `Install ${preview.skillName}`,
          subtitle: 'Review the GitHub skill before SkillBox copies it into the managed store.',
          applyLabel: 'Install from GitHub',
          applyingLabel: 'Installing...',
          error: ''
        });
        setRemoteImport((current) => ({ ...current, open: false, value: '', error: '' }));
        setNotice('Browser preview is using a provided remote source.');
        setStatus('prototype');
        requestController.finish(requestId);
        return;
      }
      setImportReview({
        open: true,
        candidates: [remoteImportCandidate(remoteImport.mode, value)],
        errors: [],
        title: 'Import Review',
        subtitle: 'Confirm each skill type before SkillBox copies it into the managed store.',
        noticePrefix: ''
      });
      setRemoteImport((current) => ({ ...current, open: false, value: '', error: '' }));
      setNotice('Browser preview is using a provided remote source.');
      setStatus('prototype');
      requestController.finish(requestId);
      return;
    }

    try {
      if (remoteImport.mode === 'url') {
        setStatus('importing');
        setRemoteImport((current) => ({ ...current, open: false, error: '' }));
        setRemoteInstallDialog({
          open: true,
          loading: true,
          applying: false,
          preview: null,
          activePath: '',
          confirmWarnings: false,
          title: 'Review GitHub install',
          subtitle: 'Loading remote skill diff before anything is copied into SkillBox.',
          applyLabel: 'Install from GitHub',
          applyingLabel: 'Installing...',
          error: ''
        });
        await waitForNextPaint();
        if (!requestController.isCurrent(requestId)) {
          return;
        }
        const updateResult = normalizeGithubSkillCollectionUpdatePreviewResult(
          await invoke('preview_github_skill_collection_update', {
            request: { source_url: value }
          })
        );
        if (!requestController.isCurrent(requestId)) {
          return;
        }
        if (updateResult.kind === 'single_skill') {
          const result = await invoke('preview_github_remote_skill_install', {
            request: {
              source_url: value,
              target_root: null
            }
          });
          if (!requestController.isCurrent(requestId)) {
            return;
          }
          const preview = normalizeRemoteInstallPreview(result);
          setRemoteInstallDialog({
            open: true,
            loading: false,
            applying: false,
            preview,
            activePath: preview.activePath,
            confirmWarnings: false,
            title: `Install ${preview.skillName}`,
            subtitle: 'Review the GitHub skill before SkillBox copies it into the managed store.',
            applyLabel: 'Install from GitHub',
            applyingLabel: 'Installing...',
            error: ''
          });
          setRemoteImport((current) => ({ ...current, value: '', error: '' }));
          setStatus('ready');
          requestController.finish(requestId);
          return;
        }
        if (updateResult.kind === 'explicit_reference_required') {
          throw new Error(updateResult.message);
        }
        if (updateResult.kind === 'update') {
          const updatePreview = updateResult.preview;
          const collections = normalizeImportCollections([{
            ...updatePreview.collection,
            from_sha: updatePreview.from_sha || updatePreview.fromSha,
            to_sha: updatePreview.to_sha || updatePreview.toSha,
            preview_id: updatePreview.preview_id || updatePreview.previewId
          }]).map((collection) => attachGithubCollectionChanges(
            collection,
            updatePreview.changes || []
          ));
          const candidates = applyGithubCollectionChangeLocks(
            normalizeImportCandidateGroups(updatePreview.groups || [], []),
            collections
          );
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
            remoteRequestId: requestId,
            reviewMode: 'update',
            applyLabel: 'Apply collection update',
            applyingLabel: 'Updating...'
          });
          setRemoteImport((current) => ({ ...current, value: '', error: '' }));
          setRemoteInstallDialog((current) => ({ ...current, open: false, loading: false }));
          setStatus('ready');
          requestController.finish(requestId);
          return;
        }
        let routedResult;
        if (updateResult.kind === 'up_to_date') {
          routedResult = { kind: 'collection', preview: updateResult.preview };
        } else {
          routedResult = normalizeGithubSkillCollectionPreviewResult(
            await invoke('preview_github_skill_collection', {
              request: { source_url: value }
            })
          );
        }
        if (!requestController.isCurrent(requestId)) {
          return;
        }
        if (routedResult.kind === 'single_skill') {
          const result = await invoke('preview_github_remote_skill_install', {
            request: {
              source_url: value,
              target_root: null
            }
          });
          if (!requestController.isCurrent(requestId)) {
            return;
          }
          const preview = normalizeRemoteInstallPreview(result);
          setRemoteInstallDialog({
            open: true,
            loading: false,
            applying: false,
            preview,
            activePath: preview.activePath,
            confirmWarnings: false,
            title: `Install ${preview.skillName}`,
            subtitle: 'Review the GitHub skill before SkillBox copies it into the managed store.',
            applyLabel: 'Install from GitHub',
            applyingLabel: 'Installing...',
            error: ''
          });
          setRemoteImport((current) => ({ ...current, value: '', error: '' }));
          setStatus('ready');
          requestController.finish(requestId);
          return;
        }
        if (routedResult.kind === 'explicit_reference_required') {
          throw new Error(routedResult.message);
        }
        {
          const collectionResult = routedResult.preview;
          const collections = normalizeImportCollections([collectionResult.collection]);
          const candidates = normalizeImportCandidateGroups(
            collectionResult.groups || [],
            []
          );
          setImportReview({
            open: true,
            loading: false,
            candidates,
            collections,
            errors: collectionResult.errors || collectionResult.collection?.errors || [],
            scanError: '',
            scanProgress: null,
            diagnostics: collectionResult.diagnostics || null,
            title: 'GitHub Collection Review',
            subtitle: 'Review selected skills from one repository snapshot before SkillBox writes managed state.',
            noticePrefix: '',
            remoteRequestId: requestId,
            reviewMode: 'install',
            applyLabel: 'Import selected',
            applyingLabel: 'Importing...'
          });
          setRemoteImport((current) => ({ ...current, value: '', error: '' }));
          setRemoteInstallDialog((current) => ({ ...current, open: false, loading: false }));
          setStatus('ready');
          requestController.finish(requestId);
          return;
        }
      } else {
        setNotice('Markdown file import is not wired yet.');
      }
    } catch (submitError) {
      if (!requestController.isCurrent(requestId)) {
        return;
      }
      setRemoteImport((current) => ({
        ...current,
        open: mode !== 'url',
        error: submitError.message || String(submitError) || 'Unable to prepare this import.'
      }));
      setRemoteInstallDialog((current) => ({ ...current, loading: false, error: submitError.message || String(submitError) }));
      setStatus('ready');
      requestController.finish(requestId);
      return;
    }

    setRemoteImport((current) => ({ ...current, open: false, value: '', error: '' }));
    setStatus('ready');
    requestController.finish(requestId);
  }

  async function refreshUsageHookStatuses(options = {}) {
    const { refresh, setError, setNotice, setUsageHooks, status } = getCtx();
    const silent = Boolean(options.silent);

    if (!window.__TAURI_INTERNALS__) {
      setUsageHooks(normalizeUsageHookStatuses(null));
      if (!silent) {
        setNotice('Usage hook status refreshed.');
      }
      return;
    }

    try {
      const hookRows = await invoke('usage_hook_statuses');
      setUsageHooks(normalizeUsageHookStatuses(hookRows));
      if (!silent) {
        setNotice('Usage hook status refreshed.');
      }
    } catch (hookError) {
      if (!silent) {
        setError(hookError.message || String(hookError) || 'Unable to refresh usage hook status.');
      }
    }
  }

  async function openUsageHookConfig(path) {
    const { setNotice } = getCtx();
    const configPath = String(path || '').trim();
    if (!configPath) {
      setNotice('No usage hook config file is available.');
      return;
    }

    if (window.__TAURI_INTERNALS__) {
      try {
        await invoke('open_local_file', { path: configPath });
        return;
      } catch (viewError) {
        setNotice(viewError.message || String(viewError));
        return;
      }
    }

    setNotice(`Usage hook config: ${compactPath(configPath)}`);
  }

  function closeSyncDialog() {
    const { error, setSyncDialog, status, syncDialog } = getCtx();
    if (status === 'syncing' || status === 'preparing_sync' || syncDialog.generating) {
      return;
    }
    setSyncDialog((current) => ({ ...current, open: false, error: '' }));
  }

  function updateSyncDialog(patch) {
    const { error, setSyncDialog } = getCtx();
    setSyncDialog((current) => ({
      ...current,
      ...patch,
      commitMessageEdited: Object.prototype.hasOwnProperty.call(patch, 'commitMessage')
        ? true
        : current.commitMessageEdited,
      generateSource: Object.prototype.hasOwnProperty.call(patch, 'commitMessage')
        ? ''
        : current.generateSource,
      error: ''
    }));
  }

  function setSyncDialogProgress({ push, selectedCount }) {
    const { error, setSyncDialog } = getCtx();
    setSyncDialog((current) => ({
      ...current,
      error: '',
      syncLog: userSkillsSyncProgressSteps({ push, selectedCount })
    }));
  }

  function toggleSyncDialogPath(path, selected) {
    const { error, filter, setSyncDialog } = getCtx();
    setSyncDialog((current) => {
      const selectedPaths = selected
        ? [...new Set([...current.selectedPaths, path])]
        : current.selectedPaths.filter((item) => item !== path);

      return {
        ...current,
        selectedPaths,
        activePath: path,
        commitMessage: current.commitMessageEdited
          ? current.commitMessage
          : suggestUserSkillsCommitMessage(current.changes.files, selectedPaths),
        error: ''
      };
    });
  }

  function selectAllSyncDialogPaths(selected) {
    const { error, setSyncDialog } = getCtx();
    setSyncDialog((current) => ({
      ...current,
      selectedPaths: selected ? current.changes.files.map((file) => file.path) : [],
      activePath: current.activePath || current.changes.files[0]?.path || '',
      commitMessage: current.commitMessageEdited
        ? current.commitMessage
        : suggestUserSkillsCommitMessage(
            current.changes.files,
            selected ? current.changes.files.map((file) => file.path) : []
          ),
      error: ''
    }));
  }

  async function submitSyncSetup(event) {
    const { error, runUserSkillsSync, setSyncDialog, syncDialog } = getCtx();
    event.preventDefault();
    const remoteUrl = syncDialog.remoteUrl.trim();
    if (syncDialog.push && !remoteUrl) {
      setSyncDialog((current) => ({
        ...current,
        error: 'Configure a Git remote URL in Settings before syncing.'
      }));
      return;
    }

    if (syncDialog.changes.files.length === 0) {
      setSyncDialog((current) => ({ ...current, error: 'No changed files to commit.' }));
      return;
    }

    const selectedPaths =
      syncDialog.changes.files.length > 0 ? syncDialog.selectedPaths : null;
    if (syncDialog.changes.files.length > 0 && selectedPaths.length === 0) {
      setSyncDialog((current) => ({ ...current, error: 'Select at least one file to commit.' }));
      return;
    }

    await runUserSkillsSync({
      remoteUrl,
      commitMessage:
        syncDialog.commitMessage ||
        suggestUserSkillsCommitMessage(syncDialog.changes.files, syncDialog.selectedPaths),
      push: syncDialog.push,
      selectedPaths,
      selectedCount: selectedPaths?.length || 0,
      closeDialog: true
    });
  }

  async function runUserSkillsSync({
    remoteUrl = '',
    commitMessage = syncCommitMessage,
    push = true,
    selectedPaths = null,
    selectedCount = selectedPaths?.length || 0,
    closeDialog = false
  } = {}) {
    const { authoritativeGenerationRef, error, setError, setNotice, setStatus, setSyncCommitMessage, setSyncDialog, setSyncDialogProgress, setUserSkillsGit, setUserSkillsInbound, skills, userSkillsGit } = getCtx();
    const generation = authoritativeGenerationRef.current + 1;
    authoritativeGenerationRef.current = generation;
    setStatus('syncing');
    setError('');
    setNotice('');
    if (closeDialog) {
      setSyncDialogProgress({ push, selectedCount });
      await waitForNextPaint();
    }

    const message = commitMessage.trim() || defaultSyncCommitMessage;

    if (!window.__TAURI_INTERNALS__) {
      const normalized = normalizeUserSkillsGitStatus({
        repo_path: previewPaths.userSkillsRoot,
        remote_url: remoteUrl || userSkillsGit.remoteUrl || 'git@example.com:santosli/user-skills.git',
        branch: 'main',
        state: 'clean',
        dirty: false,
        message: 'Mock synced user skills.'
      });
      if (generation !== authoritativeGenerationRef.current) {
        return;
      }
      setUserSkillsGit(normalized);
      setUserSkillsInbound(null);
      setSyncCommitMessage(message);
      if (closeDialog) {
        setSyncDialog((current) => ({ ...current, open: false, error: '' }));
      }
      setNotice(syncNotice(normalized));
      setStatus('prototype');
      return;
    }

    try {
      const result = await invoke('sync_user_skills_git', {
        request: {
          remote_url: null,
          commit_message: message,
          push,
          selected_paths: selectedPaths
        }
      });
      const normalized = normalizeUserSkillsGitStatus({
        ...result,
        remote_url: result.remote_url || remoteUrl || userSkillsGit.remoteUrl
      });
      if (generation !== authoritativeGenerationRef.current) {
        return;
      }
      setUserSkillsGit(normalized);
      setUserSkillsInbound(null);
      setSyncCommitMessage(message);
      if (closeDialog) {
        setSyncDialog((current) => ({ ...current, open: false, error: '' }));
      }
      setNotice(result.message || syncNotice(normalized));
      setStatus('ready');
    } catch (syncError) {
      if (generation !== authoritativeGenerationRef.current) {
        return;
      }
      const syncMessage = syncError.message || String(syncError) || 'Unable to sync user skills.';
      if (closeDialog) {
        setSyncDialog((current) => ({ ...current, error: syncMessage }));
      } else {
        setError(syncMessage);
      }
      setStatus('ready');
    }
  }

  function closeUserSkillsInboundReview() {
    const { error, inboundReviewDialog, inboundReviewRequestControllerRef, setInboundReviewDialog, setStatus, status } = getCtx();
    inboundReviewRequestControllerRef.current.cancel();
    setInboundReviewDialog((current) =>
      current.applying ? current : { ...current, open: false, loading: false, error: '' }
    );
    if (!inboundReviewDialog.applying && status === 'previewing_inbound') {
      setStatus(window.__TAURI_INTERNALS__ ? 'ready' : 'prototype');
    }
  }

  async function applyUserSkillsInbound() {
    const { authoritativeGenerationRef, error, filter, inboundReviewDialog, isFirstUse, paths, refresh, setInboundReviewDialog, setIsFirstUse, setNotice, setPaths, setSkills, setStatus, setUserSkillsGit, setUserSkillsInbound, setUserSkillsInboundWarnings, skills, status } = getCtx();
    const previewId = inboundReviewDialog.preview?.previewId;
    if (!previewId || !inboundReviewDialog.preview?.canApply) return;

    const generation = authoritativeGenerationRef.current + 1;
    authoritativeGenerationRef.current = generation;
    setStatus('applying_inbound');
    setInboundReviewDialog((current) => ({ ...current, applying: true, error: '' }));

    if (!window.__TAURI_INTERNALS__) {
      const nextStatus = normalizeUserSkillsInboundStatus({
        ...previewUserSkillsInboundStatus(),
        relation: 'synced',
        behind_count: 0,
        message: 'User skills fast-forwarded to origin/main.'
      });
      if (generation !== authoritativeGenerationRef.current) {
        return;
      }
      setUserSkillsInbound(nextStatus);
      setInboundReviewDialog((current) => ({ ...current, open: false, applying: false }));
      setNotice(nextStatus.message);
      setStatus('prototype');
      return;
    }

    let result;
    try {
      result = await invoke('apply_user_skills_inbound', {
        request: { preview_id: previewId, actor: 'desktop' }
      });
    } catch (applyError) {
      if (generation !== authoritativeGenerationRef.current) {
        return;
      }
      const applyMessage =
        applyError?.message ||
        String(applyError) ||
        'Unable to apply incoming user skills.';
      setInboundReviewDialog((current) => ({
        ...current,
        applying: false,
        preview: invalidateUserSkillsInboundPreview(current.preview),
        error: `${applyMessage} Refresh to review the current repository state.`
      }));
      setStatus('ready');
      return;
    }

    if (generation !== authoritativeGenerationRef.current) {
      return;
    }
    const changedCount = result.changed_skill_count ?? result.changedSkillCount ?? 0;
    const appliedStatus = appliedUserSkillsInboundStatus(result);
    setUserSkillsInbound(appliedStatus);
    setUserSkillsGit((current) =>
      normalizeUserSkillsGitStatus({
        ...current,
        dirty: false,
        repoPath: appliedStatus.repoPath || current.repoPath,
        state: 'clean'
      })
    );
    setInboundReviewDialog((current) => ({ ...current, open: false, applying: false }));
    setUserSkillsInboundWarnings((current) =>
      appendUserSkillsInboundWarnings(current, result.warnings)
    );
    setNotice(
      `Applied ${changedCount} incoming skill change${changedCount === 1 ? '' : 's'} by fast-forward.`
    );
    setStatus('ready');

    const [managedStateRefresh, gitStatusRefresh, inboundStatusRefresh] =
      await Promise.allSettled([
        invoke('managed_state'),
        invoke('user_skills_git_status'),
        invoke('check_user_skills_inbound')
      ]);
    if (generation !== authoritativeGenerationRef.current) {
      return;
    }

    if (managedStateRefresh.status === 'fulfilled') {
      const state = managedStateRefresh.value;
      setSkills(state.skills?.map(normalizeSkill) || []);
      setPaths(normalizePaths(state.paths));
      setIsFirstUse(Boolean(state.isFirstUse ?? state.is_first_use));
    }
    if (gitStatusRefresh.status === 'fulfilled') {
      setUserSkillsGit(normalizeUserSkillsGitStatus(gitStatusRefresh.value));
    }
    if (inboundStatusRefresh.status === 'fulfilled' && inboundStatusRefresh.value) {
      setUserSkillsInbound(normalizeUserSkillsInboundStatus(inboundStatusRefresh.value));
    }

    const refreshWarning = inboundApplyRefreshWarning(
      [
        ['Managed state refresh', managedStateRefresh],
        ['Git status refresh', gitStatusRefresh],
        ['Inbound status refresh', inboundStatusRefresh]
      ]
        .filter(([, refresh]) => refresh.status === 'rejected')
        .map(([label, refresh]) => ({ label, error: refresh.reason }))
    );
    if (refreshWarning) {
      setUserSkillsInboundWarnings((current) =>
        appendUserSkillsInboundWarnings(current, [refreshWarning])
      );
    }
  }

  async function syncLocalUsageHistories() {
    const { history, loadUsageRankings, pageRef, refresh, setError, setUsageBackfillLoading, setUsageBackfillNotice, usageRankingFilters } = getCtx();
    if (pageRef.current !== 'rankings') return;
    setUsageBackfillLoading(true);
    setError('');
    setUsageBackfillNotice('');
    try {
      const providerResults = [];
      for (const provider of usageHistorySyncProviders) {
        if (pageRef.current !== 'rankings') return;
        try {
          const result = window.__TAURI_INTERNALS__
            ? await invoke(provider.command, { request: provider.request })
            : {
                scanned_files: provider.id === 'cursor' ? 4 : 2,
                discovered: provider.id === 'codex' ? 3 : 1,
                recorded: provider.id === 'codex' ? 3 : 1,
                deduplicated: 0,
                skipped: 0,
                errors: []
              };
          providerResults.push({ provider: provider.label, ...result });
        } catch (providerError) {
          providerResults.push({
            provider: provider.label,
            errors: [
              providerError.message
                || String(providerError)
                || `${provider.label} history sync failed.`
            ]
          });
        }
      }
      if (pageRef.current !== 'rankings') return;
      const normalizedResults = providerResults.map((result) => ({
        provider: result.provider,
        ...normalizeCodexUsageBackfill(result)
      }));
      const errorCount = normalizedResults.reduce(
        (total, result) => total + result.errors.length,
        0
      );
      const syncNotice = usageHistorySyncNotice(providerResults);
      const partialWarning = errorCount > 0
        ? `Local history sync completed with ${errorCount} error${
            errorCount === 1 ? '' : 's'
          }: ${syncNotice}`
        : '';
      if (errorCount > 0) {
        setUsageBackfillNotice('');
      } else {
        setUsageBackfillNotice(syncNotice);
      }
      const rankingRefreshError = await loadUsageRankings(usageRankingFilters, {
        clearError: !partialWarning,
        reportError: !partialWarning
      });
      if (partialWarning && pageRef.current === 'rankings') {
        setError(
          rankingRefreshError
            ? `${partialWarning} Rankings refresh failed: ${rankingRefreshError}`
            : partialWarning
        );
      }
    } catch (backfillError) {
      if (pageRef.current !== 'rankings') return;
      setError(
        backfillError.message
          || String(backfillError)
          || 'Unable to import local agent usage history.'
      );
    } finally {
      setUsageBackfillLoading(false);
    }
  }

  return {
    refresh,
    refreshSkillStatuses,
    openRemoteImport,
    closeRemoteImport,
    updateRemoteImport,
    submitRemoteImport,
    refreshUsageHookStatuses,
    openUsageHookConfig,
    closeSyncDialog,
    updateSyncDialog,
    setSyncDialogProgress,
    toggleSyncDialogPath,
    selectAllSyncDialogPaths,
    submitSyncSetup,
    runUserSkillsSync,
    closeUserSkillsInboundReview,
    applyUserSkillsInbound,
    syncLocalUsageHistories
  };
}
