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
import { SettingsPage } from './components/settings.jsx';
import {
  ImportRevertDialog,
  SkillDeleteDialog,
  SkillDetailDialog,
  SkillTypeChangeDialog
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
  normalizeImportCandidate,
  selectedImportCollectionRequests,
  selectedImportCandidates,
  selectImportCandidateVariant,
  toggleImportCandidateGroup,
  toggleImportCandidateGroupSelection,
  updateImportCandidateGroupType
} from './importCandidates.js';
import {
  browserImportScanOptions,
  isImportScanRequestCurrent,
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

export default function App() {
  const [skills, setSkills] = useState([]);
  const [workspaces, setWorkspaces] = useState([]);
  const [paths, setPaths] = useState(null);
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState('all');
  const [dashboardTagFilter, setDashboardTagFilter] = useState('all');
  const [dashboardFavoritesOnly, setDashboardFavoritesOnly] = useState(false);
  const [dashboardViewMode, setDashboardViewMode] = useState('grid');
  const [workspaceTypeFilter, setWorkspaceTypeFilter] = useState('all');
  const [workspaceQuery, setWorkspaceQuery] = useState('');
  const [historyFilter, setHistoryFilter] = useState('all');
  const [usageRankingFilters, setUsageRankingFilters] = useState(defaultUsageRankingFilters);
  const [favoriteNames, setFavoriteNames] = useState(readDashboardFavorites);
  const [dashboardTagOverrides, setDashboardTagOverrides] = useState(readDashboardTagOverrides);
  const [selectedName, setSelectedName] = useState('');
  const [page, setPage] = useState('dashboard');
  const [status, setStatus] = useState('idle');
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [isFirstUse, setIsFirstUse] = useState(false);
  const [importReview, setImportReview] = useState({
    open: false,
    loading: false,
    candidates: [],
    collections: [],
    errors: [],
    scanError: '',
    scanProgress: null,
    diagnostics: null,
    title: 'Import Review',
    subtitle: 'Confirm each skill type before SkillBox copies it into the managed store.',
    noticePrefix: ''
  });
  const [preferences, setPreferences] = useState({
    skipLocalImportConfirmation: false,
    statusRefreshIntervalMinutes: 5,
    remoteUpdateTimeoutSeconds: 30
  });
  const [localImportConfirmation, setLocalImportConfirmation] = useState({
    open: false,
    candidates: [],
    collectionRequests: [],
    noticePrefix: ''
  });
  const [remoteImport, setRemoteImport] = useState({
    open: false,
    mode: 'url',
    value: '',
    error: ''
  });
  const [userSkillsGit, setUserSkillsGit] = useState(normalizeUserSkillsGitStatus(null));
  const [userSkillsInbound, setUserSkillsInbound] = useState(null);
  const [userSkillsInboundWarnings, setUserSkillsInboundWarnings] = useState([]);
  const [usageHooks, setUsageHooks] = useState(normalizeUsageHookStatuses(null));
  const [doctorReport, setDoctorReport] = useState(normalizeDoctorReport(null));
  const [remoteSkillUpdates, setRemoteSkillUpdates] = useState(normalizeRemoteSkillUpdates(null));
  const [lastStatusCheckedAt, setLastStatusCheckedAt] = useState('');
  const [syncDialog, setSyncDialog] = useState({
    open: false,
    loading: false,
    remoteUrl: '',
    commitMessage: defaultSyncCommitMessage,
    commitMessageEdited: false,
    push: true,
    error: '',
    syncLog: [],
    changes: normalizeUserSkillsGitChanges(null),
    selectedPaths: [],
    activePath: ''
  });
  const [syncCommitMessage, setSyncCommitMessage] = useState(defaultSyncCommitMessage);
  const [inboundReviewDialog, setInboundReviewDialog] = useState({
    open: false,
    loading: false,
    applying: false,
    preview: null,
    activePath: '',
    error: ''
  });
  const inboundReviewRequestControllerRef = useInboundReviewRequestController();
  const [workspaceDialog, setWorkspaceDialog] = useState({
    open: false,
    path: '',
    kind: 'user',
    error: '',
    preview: null,
    selectedRoot: ''
  });
  const workspacePreviewRequestRef = useRef(0);
  const [deployDialog, setDeployDialog] = useState({
    open: false,
    skillName: '',
    rows: [],
    confirmUndeploy: false,
    error: ''
  });
  const [skillTypeChangeDialog, setSkillTypeChangeDialog] = useState({
    open: false,
    skillName: '',
    currentType: '',
    targetType: '',
    loading: false,
    error: ''
  });
  const [remoteSourceDialog, setRemoteSourceDialog] = useState({
    open: false,
    skillName: '',
    sourceUrl: '',
    candidates: [],
    searched: false,
    searching: false,
    searchError: '',
    preview: null,
    error: '',
    loading: false,
    binding: false,
    candidateBind: closedRemoteSourceCandidateBind
  });
  const [remoteVersionDialog, setRemoteVersionDialog] = useState({
    open: false,
    loading: false,
    applying: false,
    preview: null,
    activePath: '',
    error: ''
  });
  const [remoteInstallDialog, setRemoteInstallDialog] = useState({
    open: false,
    loading: false,
    applying: false,
    preview: null,
    activePath: '',
    confirmWarnings: false,
    error: ''
  });
  const [remoteVersions, setRemoteVersions] = useState({});
  const [userVersions, setUserVersions] = useState({});
  const [importRecords, setImportRecords] = useState({});
  const [importRecordLoading, setImportRecordLoading] = useState({});
  const [operationHistory, setOperationHistory] = useState({});
  const [importRevertDialog, setImportRevertDialog] = useState({
    open: false,
    record: null,
    loading: false,
    error: ''
  });
  const [skillDeleteDialog, setSkillDeleteDialog] = useState({
    open: false,
    skillName: '',
    preview: null,
    previewLoading: false,
    confirmation: '',
    loading: false,
    error: ''
  });
  const [history, setHistory] = useState(normalizeHistory(null));
  const [usageRankings, setUsageRankings] = useState(normalizeUsageRankings(null));
  const [usageRankingLoading, setUsageRankingLoading] = useState(false);
  const [usageBackfillLoading, setUsageBackfillLoading] = useState(false);
  const [usageBackfillNotice, setUsageBackfillNotice] = useState('');
  const [rankingImportSkillName, setRankingImportSkillName] = useState('');
  const [remoteContextLoading, setRemoteContextLoading] = useState({});
  const [userContextLoading, setUserContextLoading] = useState({});
  const [appUpdate, setAppUpdate] = useState(() =>
    normalizeAppUpdateStatus(null, desktopPackage.version)
  );
  const appUpdateInstallBlocked =
    autoRefreshBlockedStatuses.has(status) ||
    remoteVersionDialog.applying ||
    remoteInstallDialog.applying ||
    remoteSourceDialog.binding ||
    remoteSourceDialog.candidateBind.binding;
  const contentRef = useRef(null);
  const autoRefreshStateRef = useRef({ status: 'idle', isFirstUse: false });
  const refreshSkillStatusesRef = useRef(null);
  const appUpdateAutoCheckedRef = useRef(false);
  const usageRankingRequestRef = useRef(0);
  const rankingImportRequestRef = useRef(0);
  const historyRequestRef = useRef(0);
  const importScanRequestRef = useRef(0);
  const importScanActiveRef = useRef(0);
  const importScanTimingRef = useRef(null);
  const authoritativeGenerationRef = useRef(0);
  const pageRef = useRef(page);
  const dismissNotice = () => setNotice('');
  const lastStatusCheckedLabel = useMemo(
    () => formatStatusCheckedAt(lastStatusCheckedAt),
    [lastStatusCheckedAt]
  );

  useEffect(() => {
    refresh();
  }, []);

  useEffect(() => {
    pageRef.current = page;
  }, [page]);

  useEffect(() => {
    if (!window.__TAURI_INTERNALS__) {
      return undefined;
    }

    let active = true;
    let unlisten;
    listen('skillbox://import-scan-progress', (event) => {
      const progress = event.payload || {};
      if (!active || !isImportScanRequestCurrent(progress.scanId, importScanRequestRef.current)) {
        return;
      }
      setImportReview((current) => current.open && current.loading
        ? { ...current, scanProgress: progress }
        : current);
    }).then((removeListener) => {
      unlisten = removeListener;
    }).catch(() => {});

    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!window.__TAURI_INTERNALS__) {
      const previewStatus = import.meta.env.DEV
        ? previewAppUpdateStatus(window.location.search, desktopPackage.version)
        : null;
      setAppUpdate(
        previewStatus ||
          normalizeAppUpdateStatus(
            {
              disabled: true,
              current_version: desktopPackage.version,
              message: 'App updater is disabled in browser preview.'
            },
            desktopPackage.version
          )
      );
    }
  }, []);

  useEffect(() => {
    autoRefreshStateRef.current = { status, isFirstUse };
  }, [status, isFirstUse]);

  useEffect(() => {
    refreshSkillStatusesRef.current = () => refreshSkillStatuses({ automatic: true });
  });

  useEffect(() => {
    if (appUpdateAutoCheckedRef.current) {
      return;
    }

    if (
      shouldCheckAppUpdateOnStartup({
        tauriAvailable: Boolean(window.__TAURI_INTERNALS__),
        updateStatus: appUpdate
      })
    ) {
      appUpdateAutoCheckedRef.current = true;
      checkAppUpdate({ automatic: true });
    }
  }, [appUpdate]);

  useEffect(() => {
    if (!window.__TAURI_INTERNALS__) {
      return undefined;
    }

    const intervalId = window.setInterval(() => {
      if (appUpdate.state === 'checking' || appUpdate.state === 'installing') {
        return;
      }
      checkAppUpdate({ automatic: true });
    }, 60 * 60 * 1000);

    return () => window.clearInterval(intervalId);
  }, [appUpdate.state]);

  useEffect(() => {
    const intervalMinutes = normalizeStatusRefreshIntervalMinutes(
      preferences.statusRefreshIntervalMinutes
    );
    const intervalId = window.setInterval(() => {
      const current = autoRefreshStateRef.current;

      if (current.isFirstUse || autoRefreshBlockedStatuses.has(current.status)) {
        return;
      }

      refreshSkillStatusesRef.current?.();
    }, intervalMinutes * 60 * 1000);

    return () => window.clearInterval(intervalId);
  }, [preferences.statusRefreshIntervalMinutes]);

  useEffect(() => {
    if (contentRef.current) {
      contentRef.current.scrollTop = 0;
      contentRef.current.scrollLeft = 0;
    }
  }, [page, filter, workspaceTypeFilter, workspaceQuery]);

  useEffect(() => {
    if (page === 'settings') {
      refreshUsageHookStatuses({ silent: true });
    }
  }, [page]);

  const favoriteNameSet = useMemo(() => new Set(favoriteNames), [favoriteNames]);
  const dashboardSkills = useMemo(
    () =>
      skills.map((skill) =>
        deriveDashboardSkill(
          skill,
          userSkillsGit,
          remoteSkillUpdates,
          favoriteNameSet,
          dashboardTagOverrides,
          workspaces
        )
      ),
    [skills, userSkillsGit, remoteSkillUpdates, favoriteNameSet, dashboardTagOverrides, workspaces]
  );
  const dashboardOptions = useMemo(
    () => dashboardFilterOptions(dashboardSkills),
    [dashboardSkills]
  );
  const workspaceSummary = useMemo(() => workspaceCounts(workspaces), [workspaces]);
  const workspaceTabs = useMemo(() => workspaceTypeTabs(workspaceSummary), [workspaceSummary]);
  const filteredWorkspaces = useMemo(
    () => workspaces.filter((workspace) => workspaceMatchesFilters(workspace, {
      query: workspaceQuery,
      type: workspaceTypeFilter
    })),
    [workspaceQuery, workspaceTypeFilter, workspaces]
  );
  const filtered = useMemo(
    () =>
      sortDashboardSkills(
        dashboardSkills.filter((skill) =>
          skillMatchesDashboardFilters(skill, {
            type: filter,
            query,
            tag: dashboardTagFilter,
            favoritesOnly: dashboardFavoritesOnly,
            remoteSkillUpdates
          })
        )
      ),
    [
      dashboardFavoritesOnly,
      dashboardSkills,
      dashboardTagFilter,
      filter,
      query,
      remoteSkillUpdates
    ]
  );

  const selectedSkill = selectedName
    ? dashboardSkills.find((skill) => skill.name === selectedName)
    : null;
  const selectedRemoteUpdate = selectedSkill
    ? remoteSkillUpdates.statuses.find((item) => item.skillName === selectedSkill.name)
    : null;
  const deployDialogSkill = deployDialog.open
    ? dashboardSkills.find((skill) => skill.name === deployDialog.skillName)
    : null;

  const counts = useMemo(
    () => {
      const refreshedUpdateCount = remoteSkillUpdates.statuses.filter(
        (update) => update.state === 'update_available'
      ).length;

      return {
        total: skills.length,
        user: skills.filter((skill) => skill.type === 'user').length,
        remote: skills.filter((skill) => skill.type === 'remote').length,
        updates:
          remoteSkillUpdates.statuses.length > 0
            ? refreshedUpdateCount
            : skills.filter(hasAvailableUpdate).length
      };
    },
    [skills, remoteSkillUpdates]
  );

  useEffect(() => {
    if (dashboardTagFilter !== 'all' && !dashboardOptions.tags.includes(dashboardTagFilter)) {
      setDashboardTagFilter('all');
    }
  }, [dashboardOptions, dashboardTagFilter]);

  async function refresh() {
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
        storedSkillUserMetadata
      ] = await Promise.all([
        invoke('managed_state'),
        invoke('managed_preferences').catch(() => null),
        invoke('user_skills_git_status').catch(() => null),
        invoke('cached_remote_skill_updates').catch(() => null),
        invoke('list_workspaces').catch(() => []),
        invoke('usage_hook_statuses').catch(() => []),
        invoke('list_skill_user_metadata').catch(() => null)
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

  async function checkAppUpdate({ automatic = false } = {}) {
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

  async function installAppUpdate() {
    if (!window.__TAURI_INTERNALS__) {
      setNotice('Development preview only. Packaged release builds perform the signed update.');
      return;
    }

    if (appUpdateInstallBlocked) {
      setNotice('Finish the current SkillBox operation before installing an app update.');
      return;
    }

    setAppUpdate((current) => ({
      ...current,
      state: 'installing',
      message: ''
    }));

    try {
      const checked = normalizeAppUpdateStatus(
        await invoke('check_app_update', { force: true }),
        desktopPackage.version
      );
      if (!checked.available) {
        setAppUpdate(checked);
        setNotice(appUpdateNotice(checked) || 'SkillBox is already up to date.');
        return;
      }
      setAppUpdate({
        ...checked,
        state: 'installing'
      });
      await invoke('install_app_update');
      setNotice('App update installed. Restarting SkillBox.');
    } catch (updateError) {
      const message =
        updateError.message || String(updateError) || 'Unable to install the app update.';
      setAppUpdate((current) => ({
        ...current,
        state: current.available ? 'available' : 'error',
        message
      }));
      setError(message);
    }
  }

  async function refreshSkillStatuses({ automatic = false, skillName = '' } = {}) {
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

  async function scanForImportCandidates() {
    if (importScanActiveRef.current) {
      return;
    }

    const scanId = importScanRequestRef.current + 1;
    importScanRequestRef.current = scanId;
    importScanActiveRef.current = scanId;
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
      if (!isImportScanRequestCurrent(scanId, importScanRequestRef.current)) {
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
        if (!isImportScanRequestCurrent(scanId, importScanRequestRef.current)) {
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
        importScanActiveRef.current = 0;
        return;
      }

      if (importScanTimingRef.current) {
        importScanTimingRef.current.commandStartedAt = performance.now();
      }
      const scan = await invoke('scan_import_candidates', { scan_id: scanId });
      if (importScanTimingRef.current) {
        importScanTimingRef.current.commandFinishedAt = performance.now();
      }
      if (!isImportScanRequestCurrent(scanId, importScanRequestRef.current)) {
        return;
      }
      const workspaceRows = await invoke('list_workspaces').catch(() => []);
      if (!isImportScanRequestCurrent(scanId, importScanRequestRef.current)) {
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
      importScanActiveRef.current = 0;
    } catch (scanError) {
      if (!isImportScanRequestCurrent(scanId, importScanRequestRef.current)) {
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
      importScanActiveRef.current = 0;
    }
  }

  function openRemoteImport() {
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
    setRemoteImport((current) => ({ ...current, open: false, error: '' }));
  }

  function updateRemoteImport(patch) {
    setRemoteImport((current) => ({ ...current, ...patch, error: '' }));
  }

  async function submitRemoteImport(event) {
    event.preventDefault();

    const value = remoteImport.value.trim();
    if (!value) {
      setRemoteImport((current) => ({ ...current, error: 'Enter a skill URL or Markdown file path.' }));
      return;
    }

    if (remoteImport.mode === 'url' && !isHttpUrl(value)) {
      setRemoteImport((current) => ({ ...current, error: 'Enter a full http(s) skill URL.' }));
      return;
    }

    if (remoteImport.mode === 'markdown' && !value.toLowerCase().endsWith('.md')) {
      setRemoteImport((current) => ({ ...current, error: 'Enter a local Markdown file path ending in .md.' }));
      return;
    }

    if (!window.__TAURI_INTERNALS__) {
      if (remoteImport.mode === 'url') {
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
        const result = await invoke('preview_github_remote_skill_install', {
          request: {
            source_url: value,
            target_root: null
          }
        });
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
        return;
      } else {
        setNotice('Markdown file import is not wired yet.');
      }
    } catch (submitError) {
      setRemoteImport((current) => ({
        ...current,
        open: remoteImport.mode !== 'url',
        error: submitError.message || String(submitError) || 'Unable to prepare this import.'
      }));
      setRemoteInstallDialog((current) => ({ ...current, loading: false, error: submitError.message || String(submitError) }));
      setStatus('ready');
      return;
    }

    setRemoteImport((current) => ({ ...current, open: false, value: '', error: '' }));
    setStatus('ready');
  }

  function closeImportReview() {
    importScanRequestRef.current += 1;
    importScanActiveRef.current = 0;
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
    setImportReview((current) => ({
      ...current,
      candidates: updater(current.candidates, groupId)
    }));
  }

  function toggleAllImportCandidates() {
    setImportReview((current) => ({
      ...current,
      candidates: toggleImportCandidateGroupSelection(current.candidates)
    }));
  }

  async function importSelectedCandidates() {
    const selected = selectedImportCandidates(importReview.candidates);
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
        collectionResults.push(await invoke('apply_import_collection', {
          request: {
            collection_id: request.collectionId,
            worktree_root: request.worktreeRoot,
            preview_id: request.previewId,
            selections: request.selections.map((selection) => ({
              relative_path: selection.relativePath,
              group_id: selection.groupId,
              variant_id: selection.variantId,
              skill_type: selection.skillType
            })),
            actor: 'desktop'
          }
        }));
      }

      setImportReview({ open: false, candidates: [], collections: [], errors: [], noticePrefix: '' });
      await refresh();
      if (page === 'rankings') {
        await loadUsageRankings(usageRankingFilters);
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
      setError(importError.message || 'Unable to import selected skills.');
      setStatus('ready');
    }
  }

  function closeLocalImportConfirmation() {
    if (status === 'importing') {
      return;
    }
    setLocalImportConfirmation({ open: false, candidates: [], collectionRequests: [], noticePrefix: '' });
  }

  async function confirmLocalImport() {
    const selected = localImportConfirmation.candidates;
    const collectionRequests = localImportConfirmation.collectionRequests || [];
    const noticePrefix = localImportConfirmation.noticePrefix || '';

    setLocalImportConfirmation({ open: false, candidates: [], collectionRequests: [], noticePrefix: '' });
    await runCandidateImport(selected, noticePrefix, collectionRequests);
  }

  async function saveStatusRefreshIntervalMinutes(minutes) {
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

  async function installUsageHook(target) {
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

  async function refreshUsageHookStatuses(options = {}) {
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

  async function openSyncDialog() {
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

  function closeSyncDialog() {
    if (status === 'syncing' || status === 'preparing_sync') {
      return;
    }
    setSyncDialog((current) => ({ ...current, open: false, error: '' }));
  }

  function updateSyncDialog(patch) {
    setSyncDialog((current) => ({
      ...current,
      ...patch,
      commitMessageEdited: Object.prototype.hasOwnProperty.call(patch, 'commitMessage')
        ? true
        : current.commitMessageEdited,
      error: ''
    }));
  }

  function setSyncDialogProgress({ push, selectedCount }) {
    setSyncDialog((current) => ({
      ...current,
      error: '',
      syncLog: userSkillsSyncProgressSteps({ push, selectedCount })
    }));
  }

  function toggleSyncDialogPath(path, selected) {
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

  function activateSyncDialogPath(path) {
    setSyncDialog((current) => ({ ...current, activePath: path }));
  }

  function generateSyncDialogMessage() {
    setSyncDialog((current) => ({
      ...current,
      commitMessage: suggestUserSkillsCommitMessage(current.changes.files, current.selectedPaths),
      commitMessageEdited: false,
      error: ''
    }));
  }

  async function submitSyncSetup(event) {
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

  async function checkUserSkillsInbound() {
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

  function closeUserSkillsInboundReview() {
    inboundReviewRequestControllerRef.current.cancel();
    setInboundReviewDialog((current) =>
      current.applying ? current : { ...current, open: false, loading: false, error: '' }
    );
    if (!inboundReviewDialog.applying && status === 'previewing_inbound') {
      setStatus(window.__TAURI_INTERNALS__ ? 'ready' : 'prototype');
    }
  }

  async function applyUserSkillsInbound() {
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

  async function openUserSkillsRepository() {
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

  function navigateToPage(nextPage) {
    pageRef.current = nextPage;
    if (nextPage !== 'rankings') {
      cancelUsageRankingRequest();
    }
    if (nextPage !== 'history') {
      historyRequestRef.current += 1;
    }
    setPage(nextPage);
  }

  function openDashboard(nextFilter = filter) {
    setFilter(nextFilter);
    setSelectedName('');
    navigateToPage('dashboard');
  }

  function clearDashboardFilters() {
    setQuery('');
    setFilter('all');
    setDashboardTagFilter('all');
    setDashboardFavoritesOnly(false);
  }

  function openHistory() {
    setSelectedName('');
    navigateToPage('history');
    void loadHistory(historyFilter);
  }

  async function loadHistory(nextFilter = historyFilter) {
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
    setSelectedName('');
    navigateToPage('rankings');
    void loadUsageRankings(usageRankingFilters);
  }

  function cancelUsageRankingRequest() {
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

  async function syncLocalUsageHistories() {
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

  function openRankedSkill(skillName) {
    const skill = skills.find((candidate) => candidate.name === skillName);
    if (!skill) {
      setError(`Managed skill ${skillName} was not found. Refresh Rankings and try again.`);
      return;
    }
    openSkill(skill);
  }

  async function importRankedSkill(row) {
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

  function openSkill(skill) {
    setSelectedName(skill.name);
    void loadImportRecords(skill.name);
    if (skill.type === 'remote') {
      void loadRemoteSkillContext(skill.name);
    } else if (skill.type === 'user') {
      void loadUserSkillContext(skill.name);
    }
  }

  function closeSkillDetail() {
    setSelectedName('');
  }

  async function loadImportRecords(skillName) {
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

  function openImportRevertDialog(record) {
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

  function closeImportRevertDialog() {
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

  async function confirmImportRevert() {
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

  async function openSkillDeleteDialog(skill) {
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

  function openSkillTypeChangeDialog(skill, targetType) {
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

  function openDeployDialog(skill) {
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

  function updateDeployWarningConfirmation(canonicalPath, confirmed) {
    setDeployDialog((current) => ({
      ...current,
      rows: current.rows.map((row) =>
        row.canonicalPath === canonicalPath ? { ...row, confirmWarnings: confirmed } : row
      ),
      error: ''
    }));
  }

  function updateDeployUndeployConfirmation(confirmed) {
    setDeployDialog((current) => ({
      ...current,
      confirmUndeploy: confirmed,
      error: ''
    }));
  }

  function refreshDeployDialogRows(nextWorkspaces) {
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

  async function loadRemoteSkillContext(skillName) {
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

  async function loadUserSkillContext(skillName) {
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

  async function openRemoteSourceDialog(skill) {
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
    setRemoteSourceDialog((current) => ({ ...current, ...patch, error: '' }));
  }

  async function searchRemoteSourceCandidates(skillName) {
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

  async function viewRemoteSourceCandidate(candidate) {
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

  async function bindRemoteSourceCandidate(candidate) {
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

  function closeRemoteSourceCandidateBind() {
    setRemoteSourceDialog((current) => ({
      ...current,
      candidateBind: closedRemoteSourceCandidateBind
    }));
  }

  async function confirmRemoteSourceCandidateBind() {
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

  async function openRemoteVersionReview(skill, action, targetVersion = '') {
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

  function closeRemoteVersionDialog() {
    if (remoteVersionDialog.applying) return;
    setRemoteVersionDialog((current) => ({ ...current, open: false, error: '' }));
  }

  function activateRemoteVersionPath(path) {
    setRemoteVersionDialog((current) => ({ ...current, activePath: path }));
  }

  async function applyRemoteVersionChange() {
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

  function closeRemoteInstallDialog() {
    if (remoteInstallDialog.applying) return;
    setRemoteInstallDialog((current) => ({ ...current, open: false, error: '' }));
  }

  function activateRemoteInstallPath(path) {
    setRemoteInstallDialog((current) => ({ ...current, activePath: path }));
  }

  function updateRemoteInstallWarningConfirmation(confirmed) {
    setRemoteInstallDialog((current) => ({ ...current, confirmWarnings: confirmed, error: '' }));
  }

  async function applyRemoteInstall() {
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

  async function toggleDashboardFavorite(skillName) {
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

  async function saveUserSkillsGitRemote(remoteUrl) {
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

  async function scanWorkspaceRegistry() {
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

  async function scanWorkspaceSkills(workspace) {
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

  function openWorkspaceDialog() {
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

  async function chooseWorkspaceDialogFolder() {
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

  async function forgetWorkspaceRow(workspace) {
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
    setSyncDialog((current) => ({ ...current, open: false, error: '' }));
    navigateToPage('settings');
  }

  return (
    <main className="appShell">
      <aside className="sidebar">
        <div className="brand">
          <img className="brandMark" src={skillBoxAppIcon} alt="" aria-hidden="true" />
          <div className="brandName">
            <strong>{APP_DISPLAY_NAME}</strong>
          </div>
          {appUpdate.available ? (
            <button
              aria-label={`Update SkillBox to version ${appUpdate.version}`}
              className="sidebarUpdateButton"
              disabled={
                appUpdate.state === 'checking' ||
                appUpdate.state === 'installing' ||
                appUpdateInstallBlocked
              }
              title={`Install SkillBox v${appUpdate.version} and restart`}
              type="button"
              onClick={installAppUpdate}
            >
              {appUpdate.state === 'installing' ? 'Updating…' : 'Update'}
            </button>
          ) : null}
        </div>

        <nav className="navGroup" aria-label="Primary">
          {sidebarItems.map((item) => (
            <NavButton
              active={page === item.id}
              icon={item.icon}
              key={item.id}
              label={item.label}
              onClick={() => {
                if (item.id === 'dashboard') {
                  openDashboard('all');
                } else if (item.id === 'rankings') {
                  openRankings();
                } else if (item.id === 'history') {
                  openHistory();
                } else {
                  setSelectedName('');
                  navigateToPage(item.id);
                }
              }}
            />
          ))}
        </nav>

        <div className="sidebarFooter">
          {sidebarFooterItems.map((item) => (
            <FooterButton
              active={page === item.id}
              icon={item.icon}
              key={item.id}
              label={item.label}
              onClick={item.url ? () => openRemoteSourceUrl(item.url) : () => {
                navigateToPage(item.id);
              }}
            />
          ))}
          <div className="sidebarVersion">
            <span>Version</span>
            <strong>v{desktopPackage.version}</strong>
          </div>
        </div>
      </aside>

      <section className="content" ref={contentRef} tabIndex={-1}>
        {page === 'settings' ? (
          <SettingsPage
            appUpdate={appUpdate}
            appUpdateInstallBlocked={appUpdateInstallBlocked}
            doctorReport={doctorReport}
            paths={paths}
            preferences={preferences}
            status={status}
            usageHooks={usageHooks}
            userSkillsInboundWarnings={userSkillsInboundWarnings}
            userSkillsInbound={userSkillsInbound}
            userSkillsGit={userSkillsGit}
            onCheckUserSkillsInbound={checkUserSkillsInbound}
            onDismissUserSkillsInboundWarnings={() => setUserSkillsInboundWarnings([])}
            onCheckAppUpdate={() => checkAppUpdate()}
            onRunDoctor={runHealthCheck}
            onRepairStaleDeployments={repairStaleDeploymentRecords}
            onInstallAppUpdate={installAppUpdate}
            onOpenUsageHookConfig={openUsageHookConfig}
            onInstallUsageHook={installUsageHook}
            onRefreshUsageHooks={refreshUsageHookStatuses}
            onSaveStatusRefreshInterval={saveStatusRefreshIntervalMinutes}
            onSaveRemoteUpdateTimeout={saveRemoteUpdateTimeoutSeconds}
            onSaveUserSkillsRemote={saveUserSkillsGitRemote}
            onReviewUserSkillsInbound={openUserSkillsInboundReview}
          />
        ) : page === 'workspaces' ? (
          <WorkspacePage
            error={error}
            filter={workspaceTypeFilter}
            query={workspaceQuery}
            notice={notice}
            status={status}
            tabs={workspaceTabs}
            workspaces={filteredWorkspaces}
            onAdd={openWorkspaceDialog}
            onDismissNotice={dismissNotice}
            onFilter={setWorkspaceTypeFilter}
            onQuery={setWorkspaceQuery}
            onForget={forgetWorkspaceRow}
            onOpenSkills={scanWorkspaceSkills}
            onScan={scanWorkspaceRegistry}
          />
        ) : page === 'history' ? (
          <HistoryPage
            error={error}
            filter={historyFilter}
            history={history}
            status={status}
            onFilter={loadHistory}
            onRefresh={loadHistory}
          />
        ) : page === 'rankings' ? (
          <UsageRankingsPage
            backfilling={usageBackfillLoading}
            error={error}
            filters={usageRankingFilters}
            importingSkillName={rankingImportSkillName}
            loading={usageRankingLoading}
            notice={usageBackfillNotice || notice}
            ranking={usageRankings}
            usageHooks={usageHooks}
            workspaces={workspaces}
            onSyncHistories={syncLocalUsageHistories}
            onDismissNotice={() => {
              setUsageBackfillNotice('');
              dismissNotice();
            }}
            onFilters={loadUsageRankings}
            onImportSkill={importRankedSkill}
            onOpenSettings={() => {
              navigateToPage('settings');
            }}
            onOpenSkill={openRankedSkill}
            onRefresh={() => loadUsageRankings(usageRankingFilters)}
          />
        ) : (
          <Dashboard
            activeTag={dashboardTagFilter}
            counts={counts}
            error={error}
            filter={filter}
            filterOptions={dashboardOptions}
            filtered={filtered}
            favoritesOnly={dashboardFavoritesOnly}
            isFirstUse={isFirstUse}
            lastStatusCheckedLabel={lastStatusCheckedLabel}
            notice={notice}
            query={query}
            status={status}
            viewMode={dashboardViewMode}
            onFavoritesOnly={setDashboardFavoritesOnly}
            onClearFilters={clearDashboardFilters}
            onFilter={setFilter}
            onOpenSkill={openSkill}
            onQuery={setQuery}
            onTagFilter={setDashboardTagFilter}
            onToggleFavorite={toggleDashboardFavorite}
            onViewMode={setDashboardViewMode}
            onInstall={openRemoteImport}
            onRefresh={scanForImportCandidates}
            onRefreshStatuses={refreshSkillStatuses}
            onDismissNotice={dismissNotice}
          />
        )}
      </section>

      {(page === 'dashboard' || page === 'rankings') && selectedSkill ? (
        <SkillDetailDialog
          skill={selectedSkill}
          status={status}
          userSkillsGit={userSkillsGit}
          remoteLoading={Boolean(remoteContextLoading[selectedSkill.name])}
          userLoading={Boolean(userContextLoading[selectedSkill.name])}
          remoteUpdate={selectedRemoteUpdate}
          versions={remoteVersions[selectedSkill.name] || null}
          userVersions={userVersions[selectedSkill.name] || null}
          importRecords={importRecords[selectedSkill.name] || []}
          importRecordsLoading={Boolean(importRecordLoading[selectedSkill.name])}
          operations={operationHistory[selectedSkill.name] || []}
          onBindRemoteSource={() => openRemoteSourceDialog(selectedSkill)}
          onCheckUpdates={() => refreshSkillStatuses({ skillName: selectedSkill.name })}
          onClose={closeSkillDetail}
          onOpenDeployDialog={() => openDeployDialog(selectedSkill)}
          onOpenLocalFolder={openLocalSkillFolder}
          onOpenSourceUrl={openRemoteSourceUrl}
          onOpenSyncSetup={openSyncDialog}
          onRequestImportRevert={openImportRevertDialog}
          onRequestDelete={openSkillDeleteDialog}
          onRequestTypeChange={openSkillTypeChangeDialog}
          onReviewRollback={(version) => openRemoteVersionReview(selectedSkill, 'rollback', version.version)}
          onReviewUpdate={() => openRemoteVersionReview(selectedSkill, 'update', selectedRemoteUpdate?.latestSha || '')}
          sourceUrl={selectedRemoteUpdate?.sourceUrl || ''}
          onTagsChange={updateDashboardSkillTags}
          onToggleFavorite={toggleDashboardFavorite}
        />
      ) : null}

      {skillTypeChangeDialog.open ? (
        <SkillTypeChangeDialog
          dialog={skillTypeChangeDialog}
          onClose={closeSkillTypeChangeDialog}
          onConfirm={confirmSkillTypeChange}
        />
      ) : null}

      {importRevertDialog.open ? (
        <ImportRevertDialog
          dialog={importRevertDialog}
          onClose={closeImportRevertDialog}
          onConfirm={confirmImportRevert}
        />
      ) : null}

      {skillDeleteDialog.open ? (
        <SkillDeleteDialog
          dialog={skillDeleteDialog}
          onClose={closeSkillDeleteDialog}
          onConfirm={confirmSkillDelete}
          onConfirmationChange={(confirmation) =>
            setSkillDeleteDialog((current) => ({ ...current, confirmation }))
          }
        />
      ) : null}

      {importReview.open ? (
        <ImportReview
          groups={importReview.candidates}
          collections={importReview.collections}
          errors={importReview.errors}
          loading={importReview.loading}
          scanError={importReview.scanError}
          scanProgress={importReview.scanProgress}
          onRetry={scanForImportCandidates}
          onClose={closeImportReview}
          onImport={importSelectedCandidates}
          onToggleAll={toggleAllImportCandidates}
          onSelectVariant={(group, variant) =>
            updateImportCandidateGroup(group.id, (groups) =>
              selectImportCandidateVariant(groups, group.id, variant.id)
            )
          }
          onToggleSelected={(group) =>
            updateImportCandidateGroup(group.id, (groups) => toggleImportCandidateGroup(groups, group.id))
          }
          onTypeChange={(group, skillType) =>
            updateImportCandidateGroup(group.id, (groups) =>
              updateImportCandidateGroupType(groups, group.id, skillType)
            )
          }
          status={status}
          subtitle={importReview.subtitle}
          title={importReview.title}
        />
      ) : null}

      {remoteImport.open ? (
        <RemoteImportDialog
          error={remoteImport.error}
          mode={remoteImport.mode}
          status={status}
          value={remoteImport.value}
          onClose={closeRemoteImport}
          onModeChange={(mode) => updateRemoteImport({ mode, value: '' })}
          onSubmit={submitRemoteImport}
          onValueChange={(value) => updateRemoteImport({ value })}
        />
      ) : null}

      {localImportConfirmation.open ? (
        <LocalImportConfirmationDialog
          candidates={localImportConfirmation.candidates}
          status={status}
          onClose={closeLocalImportConfirmation}
          onConfirm={confirmLocalImport}
          onTypeChange={(candidate, skillType) =>
            setLocalImportConfirmation((current) => ({
              ...current,
              candidates: current.candidates.map((item) =>
                item.sourcePath === candidate.sourcePath ? { ...item, skillType } : item
              )
            }))
          }
        />
      ) : null}

      {syncDialog.open ? (
        <UserSkillsSyncDialog
          dialog={syncDialog}
          status={status}
          onClose={closeSyncDialog}
          onActivatePath={activateSyncDialogPath}
          onGenerateMessage={generateSyncDialogMessage}
          onOpenSettings={openSyncSettings}
          onSelectAllPaths={selectAllSyncDialogPaths}
          onSubmit={submitSyncSetup}
          onTogglePath={toggleSyncDialogPath}
          onUpdate={updateSyncDialog}
        />
      ) : null}

      {inboundReviewDialog.open ? (
        <UserSkillsInboundReviewDialog
          dialog={inboundReviewDialog}
          onActivatePath={(activePath) =>
            setInboundReviewDialog((current) => ({ ...current, activePath }))
          }
          onApply={applyUserSkillsInbound}
          onClose={closeUserSkillsInboundReview}
          onCopyRepositoryPath={copyUserSkillsRepositoryPath}
          restoreFocusFallback={contentRef.current}
          onOpenRepository={openUserSkillsRepository}
          onRefresh={openUserSkillsInboundReview}
        />
      ) : null}

      {remoteSourceDialog.open ? (
        <RemoteSourceBindingDialog
          dialog={remoteSourceDialog}
          onBind={verifyAndBindRemoteSource}
          onBindCandidate={bindRemoteSourceCandidate}
          onClose={closeRemoteSourceDialog}
          onSearch={() => searchRemoteSourceCandidates(remoteSourceDialog.skillName)}
          onUpdate={updateRemoteSourceDialog}
          onViewCandidate={viewRemoteSourceCandidate}
        />
      ) : null}

      {remoteSourceDialog.candidateBind.open ? (
        <RemoteSourceCandidateBindDialog
          dialog={remoteSourceDialog.candidateBind}
          skillName={remoteSourceDialog.skillName}
          onClose={closeRemoteSourceCandidateBind}
          onConfirm={confirmRemoteSourceCandidateBind}
        />
      ) : null}

      {remoteVersionDialog.open ? (
        <RemoteVersionReviewDialog
          dialog={remoteVersionDialog}
          onActivatePath={activateRemoteVersionPath}
          onApply={applyRemoteVersionChange}
          onClose={closeRemoteVersionDialog}
        />
      ) : null}

      {remoteInstallDialog.open ? (
        <RemoteVersionReviewDialog
          dialog={remoteInstallDialog}
          onActivatePath={activateRemoteInstallPath}
          onApply={applyRemoteInstall}
          onClose={closeRemoteInstallDialog}
          onConfirmWarningsChange={updateRemoteInstallWarningConfirmation}
        />
      ) : null}

      {deployDialog.open && deployDialogSkill ? (
        <DeployWorkspaceDialog
          dialog={deployDialog}
          skill={deployDialogSkill}
          status={status}
          onAddWorkspace={openWorkspaceDialog}
          onClose={closeDeployDialog}
          onConfirmUndeployChange={updateDeployUndeployConfirmation}
          onConfirmWarningsChange={updateDeployWarningConfirmation}
          onSubmit={submitDeployDialog}
          onToggleWorkspace={toggleDeployWorkspace}
        />
      ) : null}

      {workspaceDialog.open ? (
        <WorkspaceAddDialog
          dialog={workspaceDialog}
          status={status}
          onChooseFolder={window.__TAURI_INTERNALS__ ? chooseWorkspaceDialogFolder : null}
          onClose={closeWorkspaceDialog}
          onPreview={previewWorkspaceDialog}
          onSelectRoot={(selectedRoot) => updateWorkspaceDialog({ selectedRoot })}
          onSubmit={submitWorkspaceDialog}
          onUpdate={updateWorkspaceDialog}
        />
      ) : null}
    </main>
  );
}
