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
import { createAppActions as createAppActionsA } from './appActionsA.js';
import { createAppActions as createAppActionsB } from './appActionsB.js';
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
  const [skillCollections, setSkillCollections] = useState([]);
  const [collectionRollbackDialog, setCollectionRollbackDialog] = useState({
    open: false,
    loading: false,
    applying: false,
    preview: null,
    error: ''
  });
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
    noticePrefix: '',
    reviewMode: 'install',
    applyLabel: 'Import selected',
    applyingLabel: 'Importing...'
  });
  const [preferences, setPreferences] = useState({
    skipLocalImportConfirmation: false,
    statusRefreshIntervalMinutes: 5,
    remoteUpdateTimeoutSeconds: 30,
    commitSummaryCli: '',
    resolvedCommitSummaryCli: ''
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
    generating: false,
    generateSource: '',
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
  const [appUpdateDialog, setAppUpdateDialog] = useState({ open: false, error: '' });
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
  const importScanControllerRef = useRef(null);
  const importScanTimingRef = useRef(null);
  const remoteImportRequestControllerRef = useRef(null);
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
      const scanController = importScanControllerRef.current;
      if (!active || !scanController || !scanController.isCurrent(progress.scanId)) {
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
  const selectedGithubCollection = githubCollectionForSkill(skillCollections, selectedSkill?.name);
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

  const apiRef = useRef({});
  const getCtx = () => apiRef.current;
  const actionsA = createAppActionsA(getCtx);
  const actionsB = createAppActionsB(getCtx);
  const {
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
    syncLocalUsageHistories,
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
    openSyncSettings,
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
    copyUserSkillsRepositoryPath,
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
  } = { ...actionsA, ...actionsB };

  Object.assign(apiRef.current, {
    appUpdate,
    appUpdateAutoCheckedRef,
    appUpdateDialog,
    authoritativeGenerationRef,
    autoRefreshStateRef,
    collectionRollbackDialog,
    contentRef,
    counts,
    dashboardFavoritesOnly,
    dashboardOptions,
    dashboardSkills,
    dashboardTagFilter,
    dashboardTagOverrides,
    dashboardViewMode,
    deployDialog,
    deployDialogSkill,
    dismissNotice,
    doctorReport,
    error,
    favoriteNameSet,
    favoriteNames,
    filter,
    filtered,
    filteredWorkspaces,
    history,
    historyFilter,
    historyRequestRef,
    importRecordLoading,
    importRecords,
    importRevertDialog,
    importReview,
    importScanControllerRef,
    importScanTimingRef,
    inboundReviewDialog,
    inboundReviewRequestControllerRef,
    isFirstUse,
    lastStatusCheckedAt,
    lastStatusCheckedLabel,
    localImportConfirmation,
    notice,
    operationHistory,
    page,
    pageRef,
    paths,
    preferences,
    query,
    rankingImportRequestRef,
    rankingImportSkillName,
    refreshSkillStatusesRef,
    remoteContextLoading,
    remoteImport,
    remoteImportRequestControllerRef,
    remoteInstallDialog,
    remoteSkillUpdates,
    remoteSourceDialog,
    remoteVersionDialog,
    remoteVersions,
    selectedGithubCollection,
    selectedName,
    selectedRemoteUpdate,
    selectedSkill,
    setAppUpdate,
    setAppUpdateDialog,
    setCollectionRollbackDialog,
    setDashboardFavoritesOnly,
    setDashboardTagFilter,
    setDashboardTagOverrides,
    setDashboardViewMode,
    setDeployDialog,
    setDoctorReport,
    setError,
    setFavoriteNames,
    setFilter,
    setHistory,
    setHistoryFilter,
    setImportRecordLoading,
    setImportRecords,
    setImportRevertDialog,
    setImportReview,
    setInboundReviewDialog,
    setIsFirstUse,
    setLastStatusCheckedAt,
    setLocalImportConfirmation,
    setNotice,
    setOperationHistory,
    setPage,
    setPaths,
    setPreferences,
    setQuery,
    setRankingImportSkillName,
    setRemoteContextLoading,
    setRemoteImport,
    setRemoteInstallDialog,
    setRemoteSkillUpdates,
    setRemoteSourceDialog,
    setRemoteVersionDialog,
    setRemoteVersions,
    setSelectedName,
    setSkillCollections,
    setSkillDeleteDialog,
    setSkillTypeChangeDialog,
    setSkills,
    setStatus,
    setSyncCommitMessage,
    setSyncDialog,
    setUsageBackfillLoading,
    setUsageBackfillNotice,
    setUsageHooks,
    setUsageRankingFilters,
    setUsageRankingLoading,
    setUsageRankings,
    setUserContextLoading,
    setUserSkillsGit,
    setUserSkillsInbound,
    setUserSkillsInboundWarnings,
    setUserVersions,
    setWorkspaceDialog,
    setWorkspaceQuery,
    setWorkspaceTypeFilter,
    setWorkspaces,
    skillCollections,
    skillDeleteDialog,
    skillTypeChangeDialog,
    skills,
    status,
    syncCommitMessage,
    syncDialog,
    usageBackfillLoading,
    usageBackfillNotice,
    usageHooks,
    usageRankingFilters,
    usageRankingLoading,
    usageRankingRequestRef,
    usageRankings,
    userContextLoading,
    userSkillsGit,
    userSkillsInbound,
    userSkillsInboundWarnings,
    userVersions,
    workspaceDialog,
    workspacePreviewRequestRef,
    workspaceQuery,
    workspaceSummary,
    workspaceTabs,
    workspaceTypeFilter,
    workspaces,
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
    syncLocalUsageHistories,
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
    openSyncSettings,
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
    copyUserSkillsRepositoryPath,
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
  });

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
              onClick={requestAppUpdateInstall}
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
            onInstallAppUpdate={requestAppUpdateInstall}
            onOpenUsageHookConfig={openUsageHookConfig}
            onInstallUsageHook={installUsageHook}
            onRefreshUsageHooks={refreshUsageHookStatuses}
            onSaveStatusRefreshInterval={saveStatusRefreshIntervalMinutes}
            onSaveRemoteUpdateTimeout={saveRemoteUpdateTimeoutSeconds}
            onSaveUserSkillsRemote={saveUserSkillsGitRemote}
            onSaveCommitSummaryCli={saveCommitSummaryCli}
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
          collection={selectedGithubCollection}
          onCheckCollectionUpdate={() => checkGithubCollectionUpdate(selectedGithubCollection)}
          onRollbackCollection={() => openGithubCollectionRollback(selectedGithubCollection)}
          sourceUrl={selectedRemoteUpdate?.sourceUrl || ''}
          onTagsChange={updateDashboardSkillTags}
          onToggleFavorite={toggleDashboardFavorite}
        />
      ) : null}

      {appUpdateDialog.open ? (
        <AppUpdateConfirmDialog
          appUpdate={appUpdate}
          error={appUpdateDialog.error}
          onClose={closeAppUpdateDialog}
          onConfirm={installAppUpdate}
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

      {collectionRollbackDialog.open ? (
        <CollectionRollbackDialog
          dialog={collectionRollbackDialog}
          onClose={() => setCollectionRollbackDialog({
            open: false,
            loading: false,
            applying: false,
            preview: null,
            error: ''
          })}
          onConfirm={applyGithubCollectionRollback}
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
          onToggleCollectionSelected={(collection) =>
            setImportReview((current) => ({
              ...current,
              candidates: toggleImportCollectionSelection(current.candidates, collection)
            }))
          }
          onCollectionTypeChange={(collection, skillType) =>
            setImportReview((current) => ({
              ...current,
              candidates: updateImportCollectionType(current.candidates, collection, skillType)
            }))
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
          applyLabel={importReview.applyLabel || 'Import selected'}
          applyingLabel={importReview.applyingLabel || 'Importing...'}
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
