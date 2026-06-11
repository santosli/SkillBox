import React, { useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import desktopPackage from '../package.json';
import skillBoxAppIcon from '../src-tauri/icons/icon.png';
import { FooterButton, NavButton } from './components/common.jsx';
import { Dashboard } from './components/dashboard.jsx';
import { HistoryPage } from './components/history.jsx';
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
import { SkillDetailDialog } from './components/skillDetail.jsx';
import { UserSkillsSyncDialog } from './components/userSkillsSync.jsx';
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
import { normalizeHistory } from './historyEntries.js';
import { normalizeImportCandidate } from './importCandidates.js';
import {
  appUpdateNotice,
  normalizeAppUpdateStatus,
  shouldCheckAppUpdateOnStartup
} from './appUpdates.js';
import {
  importNotice,
  isHttpUrl,
  isImportableCandidate,
  remoteImportCandidate,
  shouldConfirmLocalImport,
  toggleImportCandidateSelection
} from './importFlow.js';
import {
  dashboardFavoriteStorageKey,
  dashboardTagStorageKey,
  normalizePreferences,
  previewPreferenceStorageKey,
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
  previewPaths,
  previewUserSkillsGitChanges,
  previewWorkspaces
} from './previewData.js';
import {
  normalizeRemoteSourceCandidates,
  normalizeRemoteSourceBindingPreview,
  normalizeRemoteVersionPreview,
  remoteVersionActionLabel
} from './remoteSkills.js';
import {
  compactPath,
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
  normalizeRemoteSkillUpdates,
  normalizeStatusRefreshIntervalMinutes
} from './skillStatusRefresh.js';
import { normalizeUsageHookStatuses } from './usageHooks.js';
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
  normalizeWorkspace,
  normalizeWorkspaces,
  sidebarFooterItems,
  sidebarItems,
  workspaceCounts,
  workspaceDeployChangeCount,
  workspaceDeploymentChanges,
  workspaceDeployPickerRows,
  workspaceDeployRequiresConfirmation,
  workspaceMatchesTypeFilter,
  workspaceSkillReviewMeta,
  workspaceTypeTabs
} from './workspaces.js';

const autoRefreshBlockedStatuses = new Set([
  'checking',
  'importing',
  'loading',
  'preparing_sync',
  'deploying_skill',
  'installing_usage_hook',
  'loading_history',
  'scanning',
  'scanning_workspace_skills',
  'scanning_workspaces',
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
  const [historyFilter, setHistoryFilter] = useState('all');
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
    candidates: [],
    errors: [],
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
    dontShowAgain: false,
    noticePrefix: ''
  });
  const [remoteImport, setRemoteImport] = useState({
    open: false,
    mode: 'url',
    value: '',
    error: ''
  });
  const [userSkillsGit, setUserSkillsGit] = useState(normalizeUserSkillsGitStatus(null));
  const [usageHooks, setUsageHooks] = useState(normalizeUsageHookStatuses(null));
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
  const [workspaceDialog, setWorkspaceDialog] = useState({
    open: false,
    path: '',
    kind: 'user',
    error: ''
  });
  const [deployDialog, setDeployDialog] = useState({
    open: false,
    skillName: '',
    rows: [],
    confirmUndeploy: false,
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
  const [remoteVersions, setRemoteVersions] = useState({});
  const [userVersions, setUserVersions] = useState({});
  const [operationHistory, setOperationHistory] = useState({});
  const [history, setHistory] = useState(normalizeHistory(null));
  const [remoteContextLoading, setRemoteContextLoading] = useState({});
  const [userContextLoading, setUserContextLoading] = useState({});
  const [appUpdate, setAppUpdate] = useState(() =>
    normalizeAppUpdateStatus(null, desktopPackage.version)
  );
  const contentRef = useRef(null);
  const autoRefreshStateRef = useRef({ status: 'idle', isFirstUse: false });
  const refreshSkillStatusesRef = useRef(null);
  const appUpdateAutoCheckedRef = useRef(false);
  const dismissNotice = () => setNotice('');
  const lastStatusCheckedLabel = useMemo(
    () => formatStatusCheckedAt(lastStatusCheckedAt),
    [lastStatusCheckedAt]
  );

  useEffect(() => {
    refresh();
  }, []);

  useEffect(() => {
    if (!window.__TAURI_INTERNALS__) {
      setAppUpdate(
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
  }, [page, filter, workspaceTypeFilter]);

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
    () => workspaces.filter((workspace) => workspaceMatchesTypeFilter(workspace, workspaceTypeFilter)),
    [workspaceTypeFilter, workspaces]
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
        usageHookRows
      ] = await Promise.all([
        invoke('managed_state'),
        invoke('managed_preferences').catch(() => null),
        invoke('user_skills_git_status').catch(() => null),
        invoke('cached_remote_skill_updates').catch(() => null),
        invoke('list_workspaces').catch(() => []),
        invoke('usage_hook_statuses').catch(() => [])
      ]);
      const managedSkills = state.skills?.map(normalizeSkill) || [];
      const cachedRemoteUpdates = normalizeRemoteSkillUpdates(cachedRemoteUpdatesResult);

      setSkills(managedSkills);
      setWorkspaces(normalizeWorkspaces(workspaceRows));
      setUsageHooks(normalizeUsageHookStatuses(usageHookRows));
      setPaths(normalizePaths(state.paths));
      setPreferences(normalizePreferences(storedPreferences));
      setUserSkillsGit(normalizeUserSkillsGitStatus(gitStatus));
      setRemoteSkillUpdates(cachedRemoteUpdates);
      setLastStatusCheckedAt(cachedRemoteUpdates.checkedAt || '');
      setIsFirstUse(Boolean(state.isFirstUse ?? state.is_first_use));
      setSelectedName((currentName) =>
        currentName && managedSkills.some((skill) => skill.name === currentName) ? currentName : ''
      );
      setStatus('ready');
    } catch (scanError) {
      setSkills([]);
      setWorkspaces(normalizeWorkspaces(previewWorkspaces));
      setPaths(previewPaths);
      setPreferences(readPreviewPreferences());
      setUserSkillsGit(normalizeUserSkillsGitStatus(null));
      setUsageHooks(normalizeUsageHookStatuses(null));
      setRemoteSkillUpdates(normalizeRemoteSkillUpdates(null));
      setLastStatusCheckedAt('');
      setIsFirstUse(true);
      setSelectedName('');
      setError('');
      setNotice(scanError.message || 'Browser preview is mocking an empty managed store.');
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
      const result = await invoke('check_app_update');
      const nextStatus = normalizeAppUpdateStatus(result, desktopPackage.version);
      setAppUpdate(nextStatus);

      if (nextStatus.available) {
        setNotice(appUpdateNotice(nextStatus));
      } else if (!automatic) {
        setNotice(appUpdateNotice(nextStatus) || nextStatus.message || 'SkillBox is up to date.');
      }
    } catch (updateError) {
      const nextStatus = normalizeAppUpdateStatus(
        {
          error: updateError.message || String(updateError) || 'Unable to check for app updates.',
          current_version: desktopPackage.version,
          checked_at: new Date().toISOString()
        },
        desktopPackage.version
      );
      setAppUpdate(nextStatus);
      if (!automatic) {
        setError(nextStatus.message);
      }
    }
  }

  async function installAppUpdate() {
    setAppUpdate((current) => ({
      ...current,
      state: 'installing',
      message: ''
    }));

    try {
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
    setStatus('checking');
    setError('');
    if (!automatic) {
      setNotice('');
    }
    await waitForNextPaint();

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

      setRemoteSkillUpdates(nextRemoteUpdates);
      setLastStatusCheckedAt(nextRemoteUpdates.checkedAt || new Date().toISOString());
      if (!automatic) {
        setNotice(dashboardStatusNotice({ userSkillsGit, remoteUpdates: nextRemoteUpdates }));
      }
      setStatus('prototype');
      return;
    }

    try {
      const remoteUpdateRequest = skillName
        ? invoke('check_remote_skill_update', {
            skillName,
            timeoutSeconds: preferences.remoteUpdateTimeoutSeconds
          })
        : invoke('check_remote_skill_updates', {
            timeoutSeconds: preferences.remoteUpdateTimeoutSeconds
          });
      const [state, gitStatus, remoteUpdatesResult] = await Promise.all([
        invoke('managed_state'),
        invoke('user_skills_git_status').catch(() => null),
        remoteUpdateRequest
      ]);
      const managedSkills = state.skills?.map(normalizeSkill) || [];
      const nextUserSkillsGit = normalizeUserSkillsGitStatus(gitStatus);
      const nextRemoteUpdates = normalizeRemoteSkillUpdates(remoteUpdatesResult);

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
      setLastStatusCheckedAt(new Date().toISOString());
      setError(refreshError.message || String(refreshError) || 'Unable to refresh skill status.');
      setStatus('ready');
    }
  }

  async function scanForImportCandidates() {
    setStatus('scanning');
    setError('');
    setNotice('');

    try {
      if (!window.__TAURI_INTERNALS__) {
        setWorkspaces(normalizeWorkspaces(previewWorkspaces));
        setImportReview({
          open: true,
          candidates: applyPreviewImportStatuses(
            previewImportCandidates.map(normalizeImportCandidate),
            skills
          ),
          errors: [],
          title: 'Import Review',
          subtitle: 'Confirm each skill type before SkillBox copies it into the managed store.',
          noticePrefix: ''
        });
        setNotice('Browser preview is using mock scan candidates.');
        setStatus('prototype');
        return;
      }

      const scan = await invoke('scan_import_candidates');
      const workspaceRows = await invoke('list_workspaces').catch(() => []);
      const candidates = (scan.candidates || []).map(normalizeImportCandidate);
      setWorkspaces(normalizeWorkspaces(workspaceRows));

      setImportReview({
        open: candidates.length > 0,
        candidates,
        errors: scan.errors || [],
        title: 'Import Review',
        subtitle: 'Confirm each skill type before SkillBox copies it into the managed store.',
        noticePrefix: ''
      });
      setNotice(candidates.length === 0 ? 'No new local skills found.' : '');
      setStatus('ready');
    } catch (scanError) {
      setError(scanError.message || 'Unable to scan local skill folders.');
      setStatus('ready');
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
        await invoke('parse_github_url', { url: value });
        setNotice('Remote URL was accepted. Remote download/import is not wired yet.');
      } else {
        setNotice('Markdown file import is not wired yet.');
      }
    } catch (submitError) {
      setRemoteImport((current) => ({
        ...current,
        error: submitError.message || String(submitError) || 'Unable to prepare this import.'
      }));
      return;
    }

    setRemoteImport((current) => ({ ...current, open: false, value: '', error: '' }));
    setStatus('ready');
  }

  function closeImportReview() {
    setImportReview((current) => ({ ...current, open: false }));
  }

  function updateImportCandidate(sourcePath, patch) {
    setImportReview((current) => ({
      ...current,
      candidates: current.candidates.map((candidate) =>
        candidate.sourcePath === sourcePath ? { ...candidate, ...patch } : candidate
      )
    }));
  }

  function toggleAllImportCandidates() {
    setImportReview((current) => ({
      ...current,
      candidates: toggleImportCandidateSelection(current.candidates)
    }));
  }

  async function importSelectedCandidates() {
    const selected = importReview.candidates.filter((candidate) => candidate.isSelected && isImportableCandidate(candidate));
    if (selected.length === 0) {
      setNotice('Select at least one candidate without conflicts to import.');
      return;
    }

    if (shouldConfirmLocalImport(selected, preferences)) {
      setLocalImportConfirmation({
        open: true,
        candidates: selected,
        dontShowAgain: false,
        noticePrefix: importReview.noticePrefix || ''
      });
      return;
    }

    await runCandidateImport(selected, importReview.noticePrefix || '');
  }

  async function runCandidateImport(selected, noticePrefix = '') {
    setStatus('importing');
    setError('');
    setNotice('');

    if (!window.__TAURI_INTERNALS__) {
      const importedSkills = selected.map(candidateToPreviewSkill);

      setSkills((current) => mergeSkills(current, importedSkills));
      setSelectedName('');
      setIsFirstUse(false);
      setImportReview({ open: false, candidates: [], errors: [], noticePrefix: '' });
      setStatus('prototype');
      setNotice(importNotice(noticePrefix, `Mock imported ${importedSkills.length} skills.`));
      return;
    }

    try {
      const result = await invoke('import_candidates', {
        items: selected.map((candidate) => ({
          source_path: candidate.sourcePath,
          skill_type: candidate.skillType,
          deploy_back_to_source: true
        }))
      });
      const importErrors = result.errors || [];

      setImportReview({ open: false, candidates: [], errors: [], noticePrefix: '' });
      await refresh();
      setNotice(
        importNotice(
          noticePrefix,
          importErrors.length > 0
            ? `Imported ${result.imported?.length || 0} skills. ${importErrors.length} item failed.`
            : `Imported ${result.imported?.length || 0} skills.`
        )
      );
    } catch (importError) {
      setError(importError.message || 'Unable to import selected skills.');
      setStatus('ready');
    }
  }

  function closeLocalImportConfirmation() {
    if (status === 'importing') {
      return;
    }
    setLocalImportConfirmation({ open: false, candidates: [], dontShowAgain: false, noticePrefix: '' });
  }

  async function confirmLocalImport() {
    const selected = localImportConfirmation.candidates;
    let noticePrefix = localImportConfirmation.noticePrefix || '';

    if (localImportConfirmation.dontShowAgain) {
      try {
        await saveSkipLocalImportConfirmation(true);
      } catch (preferenceError) {
        noticePrefix = importNotice(
          noticePrefix,
          `Preference was not saved: ${preferenceError.message || String(preferenceError)}.`
        );
      }
    }

    setLocalImportConfirmation({ open: false, candidates: [], dontShowAgain: false, noticePrefix: '' });
    await runCandidateImport(selected, noticePrefix);
  }

  async function saveSkipLocalImportConfirmation(skip) {
    if (!window.__TAURI_INTERNALS__) {
      try {
        window.localStorage.setItem(previewPreferenceStorageKey, skip ? 'true' : 'false');
      } catch {
        // Browser preview can run without durable storage; keep the session preference in React state.
      }
      const nextPreferences = { ...preferences, skipLocalImportConfirmation: skip };
      setPreferences(nextPreferences);
      return nextPreferences;
    }

    const storedPreferences = await invoke('set_skip_local_import_confirmation', { skip });
    const nextPreferences = normalizePreferences(storedPreferences);
    setPreferences(nextPreferences);
    return nextPreferences;
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
      setUserSkillsGit(normalized);
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
      setUserSkillsGit(normalized);
      setSyncCommitMessage(message);
      if (closeDialog) {
        setSyncDialog((current) => ({ ...current, open: false, error: '' }));
      }
      setNotice(result.message || syncNotice(normalized));
      setStatus('ready');
    } catch (syncError) {
      const syncMessage = syncError.message || String(syncError) || 'Unable to sync user skills.';
      if (closeDialog) {
        setSyncDialog((current) => ({ ...current, error: syncMessage }));
      } else {
        setError(syncMessage);
      }
      setStatus('ready');
    }
  }

  function openDashboard(nextFilter = filter) {
    setFilter(nextFilter);
    setSelectedName('');
    setPage('dashboard');
  }

  function openHistory() {
    setSelectedName('');
    setPage('history');
    if (!history.entries.length) {
      void loadHistory();
    }
  }

  async function loadHistory() {
    setError('');

    if (!window.__TAURI_INTERNALS__) {
      setHistory(normalizeHistory(previewHistory()));
      setStatus('prototype');
      return;
    }

    setStatus('loading_history');
    try {
      const result = await invoke('list_history', { request: { limit: 200 } });
      setHistory(normalizeHistory(result));
      setStatus('ready');
    } catch (historyError) {
      setError(historyError.message || String(historyError) || 'Unable to load history.');
      setStatus('ready');
    }
  }

  function openSkill(skill) {
    setSelectedName(skill.name);
    if (skill.type === 'remote') {
      void loadRemoteSkillContext(skill.name);
    } else if (skill.type === 'user') {
      void loadUserSkillContext(skill.name);
    }
  }

  function closeSkillDetail() {
    setSelectedName('');
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

  function toggleDeployWorkspace(canonicalPath) {
    setDeployDialog((current) => ({
      ...current,
      rows: current.rows.map((row) =>
        row.canonicalPath === canonicalPath ? { ...row, isSelected: !row.isSelected } : row
      ),
      confirmUndeploy: false,
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
        return selectedByPath.has(key) ? { ...row, isSelected: selectedByPath.get(key) } : row;
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
        await invoke('deploy_skill', {
          skillName: deployDialog.skillName,
          targetRoot: workspace.path
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
      await refreshSkillStatuses();
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

  function toggleDashboardFavorite(skillName) {
    setFavoriteNames((current) => {
      const next = current.includes(skillName)
        ? current.filter((name) => name !== skillName)
        : [...current, skillName].sort((left, right) => left.localeCompare(right));

      try {
        window.localStorage.setItem(dashboardFavoriteStorageKey, JSON.stringify(next));
      } catch {
        // Favorites are a local dashboard preference; if storage is unavailable, keep session state.
      }

      return next;
    });
  }

  function updateDashboardSkillTags(skillName, tags) {
    if (!skillName) {
      return;
    }

    setDashboardTagOverrides((current) => {
      const next = {
        ...current,
        [skillName]: normalizeEditableTags(tags)
      };

      try {
        window.localStorage.setItem(dashboardTagStorageKey, JSON.stringify(next));
      } catch {
        // Tags are a local dashboard preference; if storage is unavailable, keep session state.
      }

      return next;
    });
  }

  async function saveUserSkillsGitRemote(remoteUrl) {
    const trimmed = remoteUrl.trim();
    if (!trimmed) {
      throw new Error('Enter a Git remote URL.');
    }

    if (!window.__TAURI_INTERNALS__) {
      const normalized = normalizeUserSkillsGitStatus({
        repo_path: previewPaths.userSkillsRoot,
        remote_url: trimmed,
        branch: 'main',
        state: 'clean',
        dirty: false
      });
      setUserSkillsGit(normalized);
      setNotice('User skills remote saved.');
      return normalized;
    }

    const result = await invoke('set_user_skills_git_remote', {
      request: { remote_url: trimmed }
    });
    const normalized = normalizeUserSkillsGitStatus(result);
    setUserSkillsGit(normalized);
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
      const candidates = applyPreviewImportStatuses(
        previewCandidatesForWorkspace(workspace).map(normalizeImportCandidate),
        skills
      );

      setImportReview({
        open: true,
        candidates,
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
      const candidates = (scan.candidates || []).map(normalizeImportCandidate);

      setWorkspaces(normalizeWorkspaces(workspaceRows));
      setImportReview({
        open: true,
        candidates,
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
    setWorkspaceDialog({ open: true, path: '', kind: 'user', error: '' });
    setNotice('');
    setError('');
  }

  function closeWorkspaceDialog() {
    if (status === 'scanning_workspaces') {
      return;
    }
    setWorkspaceDialog((current) => ({ ...current, open: false, error: '' }));
  }

  function updateWorkspaceDialog(patch) {
    setWorkspaceDialog((current) => ({ ...current, ...patch, error: '' }));
  }

  async function submitWorkspaceDialog(event) {
    event.preventDefault();
    const workspacePath = workspaceDialog.path.trim();

    if (!workspacePath) {
      setWorkspaceDialog((current) => ({ ...current, error: 'Enter a workspace path.' }));
      return;
    }

    setStatus('scanning_workspaces');
    setError('');
    setNotice('');

    if (!window.__TAURI_INTERNALS__) {
      const workspace = normalizeWorkspace({
        canonical_path: workspacePath,
        path: workspacePath,
        kind: workspaceDialog.kind,
        source: 'manual',
        agent_id: workspacePath.includes('/.codex/') ? 'codex' : 'agents',
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
      setWorkspaceDialog({ open: false, path: '', kind: 'user', error: '' });
      setNotice('Workspace added.');
      setStatus('prototype');
      return;
    }

    try {
      const workspace = await invoke('add_workspace', {
        request: {
          path: workspacePath,
          kind: workspaceDialog.kind
        }
      });
      const rows = await invoke('list_workspaces').catch(() => [workspace]);
      const normalizedRows = normalizeWorkspaces(rows);
      setWorkspaces(normalizedRows);
      refreshDeployDialogRows(normalizedRows);
      setWorkspaceDialog({ open: false, path: '', kind: 'user', error: '' });
      setNotice(`Workspace added: ${normalizeWorkspace(workspace).compactPath}`);
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
    setPage('settings');
  }

  return (
    <main className="appShell">
      <aside className="sidebar">
        <div className="brand">
          <img className="brandMark" src={skillBoxAppIcon} alt="" aria-hidden="true" />
          <div>
            <strong>SkillBox</strong>
          </div>
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
                } else if (item.id === 'history') {
                  openHistory();
                } else {
                  setSelectedName('');
                  setPage(item.id);
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
              onClick={item.url ? () => openRemoteSourceUrl(item.url) : () => setPage(item.id)}
            />
          ))}
          <div className="sidebarVersion">
            <span>Version</span>
            <strong>v{desktopPackage.version}</strong>
          </div>
        </div>
      </aside>

      <section className="content" ref={contentRef}>
        {page === 'settings' ? (
          <SettingsPage
            appUpdate={appUpdate}
            paths={paths}
            preferences={preferences}
            status={status}
            usageHooks={usageHooks}
            userSkillsGit={userSkillsGit}
            onCheckAppUpdate={() => checkAppUpdate()}
            onInstallAppUpdate={installAppUpdate}
            onOpenUsageHookConfig={openUsageHookConfig}
            onInstallUsageHook={installUsageHook}
            onRefreshUsageHooks={refreshUsageHookStatuses}
            onSaveStatusRefreshInterval={saveStatusRefreshIntervalMinutes}
            onSaveRemoteUpdateTimeout={saveRemoteUpdateTimeoutSeconds}
            onSaveUserSkillsRemote={saveUserSkillsGitRemote}
          />
        ) : page === 'workspaces' ? (
          <WorkspacePage
            error={error}
            filter={workspaceTypeFilter}
            notice={notice}
            status={status}
            tabs={workspaceTabs}
            workspaces={filteredWorkspaces}
            onAdd={openWorkspaceDialog}
            onDismissNotice={dismissNotice}
            onFilter={setWorkspaceTypeFilter}
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
            onFilter={setHistoryFilter}
            onRefresh={loadHistory}
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

      {page === 'dashboard' && selectedSkill ? (
        <SkillDetailDialog
          skill={selectedSkill}
          status={status}
          userSkillsGit={userSkillsGit}
          remoteLoading={Boolean(remoteContextLoading[selectedSkill.name])}
          userLoading={Boolean(userContextLoading[selectedSkill.name])}
          remoteUpdate={selectedRemoteUpdate}
          versions={remoteVersions[selectedSkill.name] || null}
          userVersions={userVersions[selectedSkill.name] || null}
          operations={operationHistory[selectedSkill.name] || []}
          onBindRemoteSource={() => openRemoteSourceDialog(selectedSkill)}
          onCheckUpdates={() => refreshSkillStatuses({ skillName: selectedSkill.name })}
          onClose={closeSkillDetail}
          onOpenDeployDialog={() => openDeployDialog(selectedSkill)}
          onOpenLocalFolder={openLocalSkillFolder}
          onOpenSourceUrl={openRemoteSourceUrl}
          onOpenSyncSetup={openSyncDialog}
          onReviewRollback={(version) => openRemoteVersionReview(selectedSkill, 'rollback', version.version)}
          onReviewUpdate={() => openRemoteVersionReview(selectedSkill, 'update', selectedRemoteUpdate?.latestSha || '')}
          sourceUrl={selectedRemoteUpdate?.sourceUrl || ''}
          onTagsChange={updateDashboardSkillTags}
          onToggleFavorite={toggleDashboardFavorite}
        />
      ) : null}

      {importReview.open ? (
        <ImportReview
          candidates={importReview.candidates}
          errors={importReview.errors}
          onClose={closeImportReview}
          onImport={importSelectedCandidates}
          onToggleAll={toggleAllImportCandidates}
          onToggleSelected={(candidate) =>
            isImportableCandidate(candidate)
              ? updateImportCandidate(candidate.sourcePath, { isSelected: !candidate.isSelected })
              : null
          }
          onTypeChange={(candidate, skillType) => updateImportCandidate(candidate.sourcePath, { skillType })}
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
          dontShowAgain={localImportConfirmation.dontShowAgain}
          status={status}
          onClose={closeLocalImportConfirmation}
          onConfirm={confirmLocalImport}
          onDontShowAgainChange={(dontShowAgain) =>
            setLocalImportConfirmation((current) => ({ ...current, dontShowAgain }))
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

      {deployDialog.open && deployDialogSkill ? (
        <DeployWorkspaceDialog
          dialog={deployDialog}
          skill={deployDialogSkill}
          status={status}
          onAddWorkspace={openWorkspaceDialog}
          onClose={closeDeployDialog}
          onConfirmUndeployChange={updateDeployUndeployConfirmation}
          onSubmit={submitDeployDialog}
          onToggleWorkspace={toggleDeployWorkspace}
        />
      ) : null}

      {workspaceDialog.open ? (
        <WorkspaceAddDialog
          dialog={workspaceDialog}
          status={status}
          onClose={closeWorkspaceDialog}
          onSubmit={submitWorkspaceDialog}
          onUpdate={updateWorkspaceDialog}
        />
      ) : null}
    </main>
  );
}
