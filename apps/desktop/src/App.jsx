import React, { useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Grid3X3, List, RefreshCw, Search, Star, X } from 'lucide-react';
import desktopPackage from '../package.json';
import skillBoxAppIcon from '../src-tauri/icons/icon.png';
import codexAppIcon from './assets/codex-app-icon.png';
import codexCliIcon from './assets/codex-cli-icon.png';
import { dashboardTabItems, skillMatchesDashboardFilters } from './dashboardFilters.js';
import {
  dashboardFilterOptions,
  deriveDashboardSkill,
  normalizeDashboardTagOverrides,
  normalizeEditableTags,
  normalizeFavoriteNames
} from './dashboardMetadata.js';
import { parseUnifiedDiff } from './gitDiffView.js';
import { normalizeImportCandidate } from './importCandidates.js';
import {
  dashboardStatusNotice,
  formatStatusCheckedAt,
  formatStatusNoticeCountdown,
  normalizeRemoteSkillUpdates,
  normalizeStatusRefreshIntervalMinutes,
  statusNoticeAutoCloseSeconds
} from './skillStatusRefresh.js';
import {
  canCommitUserSkillsChanges,
  defaultSyncCommitMessage,
  normalizeUserSkillsGitChanges,
  normalizeUserSkillsGitStatus,
  suggestUserSkillsCommitMessage,
  syncNotice,
  userSkillsSyncProgressSteps,
  waitForNextPaint,
  userSyncAction,
  userSyncLabel
} from './userSkillsGitSync.js';

const previewPaths = {
  root: '~/SkillBox',
  userSkillsRoot: '~/SkillBox/user-skills',
  remoteSkillsRoot: '~/SkillBox/remote-skills',
  databasePath: '~/SkillBox/skillbox.sqlite'
};

const previewPreferenceStorageKey = 'skillbox.skipLocalImportConfirmation';
const previewStatusRefreshIntervalStorageKey = 'skillbox.statusRefreshIntervalMinutes';
const dashboardFavoriteStorageKey = 'skillbox.dashboardFavorites';
const dashboardTagStorageKey = 'skillbox.dashboardTags';
const autoRefreshBlockedStatuses = new Set([
  'checking',
  'importing',
  'loading',
  'preparing_sync',
  'scanning',
  'syncing'
]);

const previewImportCandidates = [
  {
    name: 'personal-wiki-updater',
    description: 'Incrementally refresh the personal wiki derived layer.',
    sourcePath: '~/.agents/skills/personal-wiki-updater',
    sourceRoot: '~/.agents/skills',
    contentHash: '87b21f5571a7d332',
    suggestedType: 'user',
    skillType: 'user',
    suggestionReason: 'inside ~/.agents/skills',
    importOrigin: 'local-scan',
    isSelected: true,
    conflict: null
  },
  {
    name: 'find-skills',
    description: 'Discover and install agent skills from local and remote sources.',
    sourcePath: '~/.codex/skills/find-skills',
    sourceRoot: '~/.codex/skills',
    contentHash: 'a9c42f1dd4822c80',
    suggestedType: 'remote',
    skillType: 'remote',
    suggestionReason: 'inside ~/.codex/skills',
    importOrigin: 'local-scan',
    isSelected: true,
    conflict: null
  },
  {
    name: 'imagegen',
    description: 'Generate and edit raster images for Codex workflows.',
    sourcePath: '~/.codex/skills/.system/imagegen',
    sourceRoot: '~/.codex/skills/.system',
    contentHash: 'c31de80b7ad93412',
    suggestedType: 'remote',
    skillType: 'remote',
    suggestionReason: 'inside ~/.codex/skills/.system',
    importOrigin: 'local-scan',
    importStatus: 'system',
    isSelected: false,
    conflict: null
  }
];

function previewUserSkillsGitChanges() {
  return {
    repo_path: previewPaths.userSkillsRoot,
    initialized: true,
    branch: 'main',
    remote_url: 'git@example.com:santosli/my-skills.git',
    files: [
      {
        path: 'codex-chat-sync/SKILL.md',
        status: ' M',
        diff:
          'diff --git a/codex-chat-sync/SKILL.md b/codex-chat-sync/SKILL.md\n' +
          '--- a/codex-chat-sync/SKILL.md\n' +
          '+++ b/codex-chat-sync/SKILL.md\n' +
          '@@\n' +
          '+description: Import Codex App history into Pandora.\n'
      },
      {
        path: 'dida-task-sync/SKILL.md',
        status: '??',
        diff:
          'diff --git a/dida-task-sync/SKILL.md b/dida-task-sync/SKILL.md\n' +
          'new file mode 100644\n' +
          '--- /dev/null\n' +
          '+++ b/dida-task-sync/SKILL.md\n' +
          '@@\n' +
          '+name: dida-task-sync\n'
      }
    ]
  };
}

export default function App() {
  const [skills, setSkills] = useState([]);
  const [paths, setPaths] = useState(null);
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState('all');
  const [dashboardTagFilter, setDashboardTagFilter] = useState('all');
  const [dashboardFavoritesOnly, setDashboardFavoritesOnly] = useState(false);
  const [dashboardViewMode, setDashboardViewMode] = useState('grid');
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
    errors: []
  });
  const [preferences, setPreferences] = useState({
    skipLocalImportConfirmation: false,
    statusRefreshIntervalMinutes: 5
  });
  const [localImportConfirmation, setLocalImportConfirmation] = useState({
    open: false,
    candidates: [],
    dontShowAgain: false
  });
  const [remoteImport, setRemoteImport] = useState({
    open: false,
    mode: 'url',
    value: '',
    error: ''
  });
  const [userSkillsGit, setUserSkillsGit] = useState(normalizeUserSkillsGitStatus(null));
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
  const contentRef = useRef(null);
  const autoRefreshStateRef = useRef({ status: 'idle', isFirstUse: false });
  const refreshSkillStatusesRef = useRef(null);
  const dismissNotice = () => setNotice('');
  const lastStatusCheckedLabel = useMemo(
    () => formatStatusCheckedAt(lastStatusCheckedAt),
    [lastStatusCheckedAt]
  );

  useEffect(() => {
    refresh();
  }, []);

  useEffect(() => {
    autoRefreshStateRef.current = { status, isFirstUse };
  }, [status, isFirstUse]);

  useEffect(() => {
    refreshSkillStatusesRef.current = () => refreshSkillStatuses({ automatic: true });
  });

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
  }, [page, filter]);

  const favoriteNameSet = useMemo(() => new Set(favoriteNames), [favoriteNames]);
  const dashboardSkills = useMemo(
    () =>
      skills.map((skill) =>
        deriveDashboardSkill(
          skill,
          userSkillsGit,
          remoteSkillUpdates,
          favoriteNameSet,
          dashboardTagOverrides
        )
      ),
    [skills, userSkillsGit, remoteSkillUpdates, favoriteNameSet, dashboardTagOverrides]
  );
  const dashboardOptions = useMemo(
    () => dashboardFilterOptions(dashboardSkills),
    [dashboardSkills]
  );
  const filtered = useMemo(
    () =>
      dashboardSkills.filter((skill) =>
        skillMatchesDashboardFilters(skill, {
          type: filter,
          query,
          tag: dashboardTagFilter,
          favoritesOnly: dashboardFavoritesOnly,
          remoteSkillUpdates
        })
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

      const [state, storedPreferences, gitStatus] = await Promise.all([
        invoke('managed_state'),
        invoke('managed_preferences').catch(() => null),
        invoke('user_skills_git_status').catch(() => null)
      ]);
      const managedSkills = state.skills?.map(normalizeSkill) || [];

      setSkills(managedSkills);
      setPaths(normalizePaths(state.paths));
      setPreferences(normalizePreferences(storedPreferences));
      setUserSkillsGit(normalizeUserSkillsGitStatus(gitStatus));
      setRemoteSkillUpdates(normalizeRemoteSkillUpdates(null));
      setLastStatusCheckedAt('');
      setIsFirstUse(Boolean(state.isFirstUse ?? state.is_first_use));
      setSelectedName((currentName) =>
        currentName && managedSkills.some((skill) => skill.name === currentName) ? currentName : ''
      );
      setStatus('ready');
    } catch (scanError) {
      setSkills([]);
      setPaths(previewPaths);
      setPreferences(readPreviewPreferences());
      setUserSkillsGit(normalizeUserSkillsGitStatus(null));
      setRemoteSkillUpdates(normalizeRemoteSkillUpdates(null));
      setLastStatusCheckedAt('');
      setIsFirstUse(true);
      setSelectedName('');
      setError('');
      setNotice(scanError.message || 'Browser preview is mocking an empty managed store.');
      setStatus('prototype');
    }
  }

  async function refreshSkillStatuses({ automatic = false } = {}) {
    setStatus('checking');
    setError('');
    if (!automatic) {
      setNotice('');
    }

    if (!window.__TAURI_INTERNALS__) {
      const nextRemoteUpdates = normalizeRemoteSkillUpdates({
        statuses: skills
          .filter((skill) => skill.type === 'remote')
          .map((skill, index) => ({
            skill_name: skill.name,
            state: index === 0 ? 'update_available' : 'up_to_date',
            update_available: index === 0
          }))
      });

      setRemoteSkillUpdates(nextRemoteUpdates);
      setLastStatusCheckedAt(new Date().toISOString());
      if (!automatic) {
        setNotice(dashboardStatusNotice({ userSkillsGit, remoteUpdates: nextRemoteUpdates }));
      }
      setStatus('prototype');
      return;
    }

    try {
      const [state, gitStatus, remoteUpdatesResult] = await Promise.all([
        invoke('managed_state'),
        invoke('user_skills_git_status').catch(() => null),
        invoke('check_remote_skill_updates')
      ]);
      const managedSkills = state.skills?.map(normalizeSkill) || [];
      const nextUserSkillsGit = normalizeUserSkillsGitStatus(gitStatus);
      const nextRemoteUpdates = normalizeRemoteSkillUpdates(remoteUpdatesResult);

      setSkills(managedSkills);
      setPaths(normalizePaths(state.paths));
      setUserSkillsGit(nextUserSkillsGit);
      setRemoteSkillUpdates(nextRemoteUpdates);
      setLastStatusCheckedAt(new Date().toISOString());
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
        setImportReview({
          open: true,
          candidates: applyPreviewImportStatuses(
            previewImportCandidates.map(normalizeImportCandidate),
            skills
          ),
          errors: []
        });
        setNotice('Browser preview is using mock scan candidates.');
        setStatus('prototype');
        return;
      }

      const scan = await invoke('scan_import_candidates');
      const candidates = (scan.candidates || []).map(normalizeImportCandidate);

      setImportReview({
        open: candidates.length > 0,
        candidates,
        errors: scan.errors || []
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
        errors: []
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
        dontShowAgain: false
      });
      return;
    }

    await runCandidateImport(selected);
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
      setImportReview({ open: false, candidates: [], errors: [] });
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

      setImportReview({ open: false, candidates: [], errors: [] });
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
    setLocalImportConfirmation({ open: false, candidates: [], dontShowAgain: false });
  }

  async function confirmLocalImport() {
    const selected = localImportConfirmation.candidates;
    let noticePrefix = '';

    if (localImportConfirmation.dontShowAgain) {
      try {
        await saveSkipLocalImportConfirmation(true);
      } catch (preferenceError) {
        noticePrefix = `Preference was not saved: ${preferenceError.message || String(preferenceError)}.`;
      }
    }

    setLocalImportConfirmation({ open: false, candidates: [], dontShowAgain: false });
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
        remote_url: remoteUrl || userSkillsGit.remoteUrl || 'git@example.com:santosli/my-skills.git',
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

  function openSkill(skill) {
    setSelectedName(skill.name);
  }

  function closeSkillDetail() {
    setSelectedName('');
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
            <span>Local skill manager</span>
          </div>
        </div>

        <nav className="navGroup" aria-label="Primary">
          <NavButton active={page === 'dashboard' && (filter === 'all' || filter === 'updates')} icon="dashboard" label="Dashboard" onClick={() => openDashboard('all')} />
          <NavButton active={page === 'dashboard' && filter === 'user'} icon="user-skills" label="User Skills" onClick={() => openDashboard('user')} />
          <NavButton active={page === 'dashboard' && filter === 'remote'} icon="remote-skills" label="Remote Skills" onClick={() => openDashboard('remote')} />
        </nav>

        <div className="sidebarFooter">
          <FooterButton active={page === 'settings'} icon="settings" label="Settings" onClick={() => setPage('settings')} />
          <FooterButton icon="help" label="Help" />
          <div className="sidebarVersion">
            <span>Version</span>
            <strong>v{desktopPackage.version}</strong>
          </div>
        </div>
      </aside>

      <section className="content" ref={contentRef}>
        {page === 'settings' ? (
          <SettingsPage
            paths={paths}
            preferences={preferences}
            status={status}
            userSkillsGit={userSkillsGit}
            onSaveStatusRefreshInterval={saveStatusRefreshIntervalMinutes}
            onSaveUserSkillsRemote={saveUserSkillsGitRemote}
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
          onCheckUpdates={refreshSkillStatuses}
          onClose={closeSkillDetail}
          onOpenSyncSetup={openSyncDialog}
          onTagsChange={updateDashboardSkillTags}
          onToggleFavorite={toggleDashboardFavorite}
        />
      ) : null}

      {importReview.open ? (
        <ImportReview
          candidates={importReview.candidates}
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
    </main>
  );
}

function Dashboard({
  activeTag,
  counts,
  error,
  favoritesOnly,
  filter,
  filterOptions,
  filtered,
  isFirstUse,
  lastStatusCheckedLabel,
  notice,
  query,
  status,
  viewMode,
  onFavoritesOnly,
  onFilter,
  onInstall,
  onOpenSkill,
  onQuery,
  onRefresh,
  onRefreshStatuses,
  onTagFilter,
  onToggleFavorite,
  onViewMode,
  onDismissNotice
}) {
  const isChecking = status === 'checking';
  const tabs = dashboardTabItems(counts);

  return (
    <>
      {error ? <div className="notice">{error}</div> : null}
      {isFirstUse && notice ? <div className="notice success">{notice}</div> : null}

      {isFirstUse ? (
        <FirstUseDashboard status={status} onInstall={onInstall} onScan={onRefresh} />
      ) : (
        <section className="dashboardFrame" aria-label="Skills dashboard">
          <div className="dashboardTitleRow">
            <div className="dashboardTitleGroup">
              <h1>Skills</h1>
              <span className="dashboardCountPill">{filtered.length}</span>
            </div>
          </div>

          <div className="dashboardControlRow">
            <label className="searchField dashboardSearch" aria-label="Search skills">
              <Search aria-hidden="true" />
              <input
                value={query}
                onChange={(event) => onQuery(event.target.value)}
                name="skill-search"
                placeholder="Search skills in SkillBox..."
                type="search"
              />
            </label>

            <div className="dashboardTypeTabs" role="tablist" aria-label="Skill type">
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

            <DashboardActionGroup
              isChecking={isChecking}
              onInstall={onInstall}
              onRefresh={onRefresh}
              onRefreshStatuses={onRefreshStatuses}
            />

            <div className="viewSwitch" role="group" aria-label="Dashboard view">
              <button
                aria-label="Show card view"
                aria-pressed={viewMode === 'grid'}
                className={viewMode === 'grid' ? 'active' : ''}
                type="button"
                onClick={() => onViewMode('grid')}
              >
                <Grid3X3 aria-hidden="true" />
              </button>
              <button
                aria-label="Show list view"
                aria-pressed={viewMode === 'list'}
                className={viewMode === 'list' ? 'active' : ''}
                type="button"
                onClick={() => onViewMode('list')}
              >
                <List aria-hidden="true" />
              </button>
            </div>
          </div>

          {notice ? (
            <DashboardStatusNotice message={notice} onDismiss={onDismissNotice} />
          ) : null}

          <div className="dashboardFilterRow" aria-label="Dashboard filters">
            <DashboardChipGroup
              active={activeTag}
              allLabel="All tags"
              label="Tags"
              options={filterOptions.tags}
              onSelect={onTagFilter}
            />
            <button
              aria-pressed={favoritesOnly}
              className={favoritesOnly ? 'favoriteFilterButton active' : 'favoriteFilterButton'}
              type="button"
              onClick={() => onFavoritesOnly(!favoritesOnly)}
            >
              <Star aria-hidden="true" />
              Favorites
            </button>
          </div>

          {viewMode === 'grid' ? (
            <div className="skillCardGrid" aria-label="Skill cards">
              {filtered.map((skill) => (
                <SkillCard
                  key={`${skill.sourceRoot}-${skill.name}`}
                  skill={skill}
                  onOpen={onOpenSkill}
                  onToggleFavorite={onToggleFavorite}
                />
              ))}
            </div>
          ) : (
            <div className="skillsTable dashboardList" role="table" aria-label="All skills">
              <div className="tableHeader" role="row">
                <span>Name</span>
                <span>Type</span>
                <span>Status</span>
                <span>Checked</span>
              </div>

              {filtered.map((skill) => (
                <button
                  className="tableRow"
                  key={`${skill.sourceRoot}-${skill.name}`}
                  type="button"
                  onClick={() => onOpenSkill(skill)}
                >
                  <span className="skillNameCell">
                    <strong>{skill.name}</strong>
                    <small>{skill.description || 'No description in SKILL.md'}</small>
                    <span className="tableTagLine">{skill.displayTags.join(', ')}</span>
                  </span>
                  <Badge tone={skill.type === 'user' ? 'green' : 'blue'}>{labelize(skill.type)}</Badge>
                  <Badge tone={skill.statusTone}>{skill.statusLabel}</Badge>
                  <span className="checkedText">{lastStatusCheckedLabel}</span>
                </button>
              ))}
            </div>
          )}

          {filtered.length === 0 ? (
            <div className="emptyState dashboardEmptyState">
              <strong>No skills found</strong>
              <span>Try another filter or run a fresh scan.</span>
            </div>
          ) : null}
        </section>
      )}
    </>
  );
}

function DashboardActionGroup({ isChecking, onInstall, onRefresh, onRefreshStatuses }) {
  return (
    <div className="dashboardActionGroup" aria-label="Skill actions">
      <button
        className="dashboardActionButton"
        disabled={isChecking}
        type="button"
        onClick={onRefreshStatuses}
      >
        <RefreshCw aria-hidden="true" />
        {isChecking ? 'Checking...' : 'Refresh status'}
      </button>
      <button className="dashboardActionButton" type="button" onClick={onRefresh}>
        Scan
      </button>
      <button className="dashboardActionButton primary" type="button" onClick={onInstall}>
        Install skill
      </button>
    </div>
  );
}

function DashboardChipGroup({ active, allLabel, label, options, onSelect }) {
  return (
    <div className="dashboardChipGroup">
      <span>{label}</span>
      <div>
        <button
          className={active === 'all' ? 'active' : ''}
          type="button"
          onClick={() => onSelect('all')}
        >
          {allLabel}
        </button>
        {options.map((option) => (
          <button
            className={active === option ? 'active' : ''}
            key={option}
            type="button"
            onClick={() => onSelect(option)}
          >
            {option}
          </button>
        ))}
      </div>
    </div>
  );
}

function SkillCard({ skill, onOpen, onToggleFavorite }) {
  return (
    <article className={skill.isFavorite ? 'skillCard favorite' : 'skillCard'}>
      <button className="skillCardHitArea" type="button" onClick={() => onOpen(skill)}>
        <span className="skillCardTitleRow">
          <strong>{skill.name}</strong>
          <Badge tone={skill.statusTone}>{skill.statusLabel}</Badge>
        </span>
        <span className="skillCardDescription">
          {skill.description || 'No description in SKILL.md'}
        </span>
        <span className="skillCardTags">
          {skill.displayTags.map((tag) => (
            <span className="tagPill" key={tag}>
              {tag}
            </span>
          ))}
        </span>
        <span className="skillCardMeta">
          <Badge tone={skill.type === 'user' ? 'green' : 'blue'}>{labelize(skill.type)}</Badge>
          <AgentIconStack agents={skill.installedAgents} />
        </span>
      </button>
      <button
        aria-label={skill.isFavorite ? `Remove ${skill.name} from favorites` : `Add ${skill.name} to favorites`}
        aria-pressed={skill.isFavorite}
        className={skill.isFavorite ? 'skillFavoriteButton active' : 'skillFavoriteButton'}
        type="button"
        onClick={() => onToggleFavorite(skill.name)}
      >
        <Star aria-hidden="true" />
      </button>
    </article>
  );
}

function AgentIconStack({ agents = [] }) {
  const visibleAgents = agents.slice(0, 4);
  const overflowCount = Math.max(agents.length - visibleAgents.length, 0);
  const label = agents.length
    ? `Installed agents: ${agents.map((agent) => agent.label).join(', ')}`
    : 'No installed agent target';

  return (
    <span className="skillAgentIcons" aria-label={label} title={label}>
      {visibleAgents.map((agent) => (
        <span className={`skillAgentIcon ${agent.id}`} key={agent.id}>
          {agent.id === 'codex' ? (
            <img src={codexCliIcon} alt="" aria-hidden="true" />
          ) : (
            <span aria-hidden="true">{agentInitial(agent)}</span>
          )}
        </span>
      ))}
      {overflowCount > 0 ? <span className="skillAgentIcon overflow">+{overflowCount}</span> : null}
    </span>
  );
}

function agentInitial(agent) {
  if (agent.id === 'claude-code') return 'CC';
  return String(agent.label || agent.id || '?').slice(0, 1).toUpperCase();
}

function DashboardStatusNotice({ message, onDismiss }) {
  const [remainingSeconds, setRemainingSeconds] = useState(statusNoticeAutoCloseSeconds);

  useEffect(() => {
    const startedAt = Date.now();

    setRemainingSeconds(statusNoticeAutoCloseSeconds);

    const intervalId = window.setInterval(() => {
      const elapsedSeconds = Math.floor((Date.now() - startedAt) / 1000);
      setRemainingSeconds(Math.max(statusNoticeAutoCloseSeconds - elapsedSeconds, 0));
    }, 250);
    const timeoutId = window.setTimeout(onDismiss, statusNoticeAutoCloseSeconds * 1000);

    return () => {
      window.clearInterval(intervalId);
      window.clearTimeout(timeoutId);
    };
  }, [message]);

  return (
    <div className="panelNotice notice success dashboardStatusNotice" role="status">
      <span className="dashboardStatusNoticeMessage">{message}</span>
      <span className="dashboardStatusNoticeCountdown">
        {formatStatusNoticeCountdown(remainingSeconds)}
      </span>
      <button
        className="noticeDismissButton"
        type="button"
        aria-label="Dismiss status notice"
        onClick={onDismiss}
      >
        <X aria-hidden="true" size={14} />
      </button>
    </div>
  );
}

function FirstUseDashboard({ status, onInstall, onScan }) {
  return (
    <section className="firstUseGrid firstUseOnly">
      <div className="panel firstUsePanel">
        <div className="emptyGlyph">
          <Icon name="dashboard" />
        </div>
        <div>
          <p className="eyebrow">First import</p>
          <h2>No skills imported yet</h2>
          <p>
            SkillBox will scan local runtime folders, show the candidates first, and only import the
            skills you confirm.
          </p>
        </div>
        <div className="firstUseActions">
          <button className="button primary" type="button" onClick={onScan}>
            {status === 'scanning' ? 'Scanning...' : 'Scan local skills'}
          </button>
          <button className="button secondary" type="button" onClick={onInstall}>
            Import from remote
          </button>
        </div>
      </div>
    </section>
  );
}

function SettingsPage({
  paths,
  preferences,
  status,
  userSkillsGit,
  onSaveStatusRefreshInterval,
  onSaveUserSkillsRemote
}) {
  return (
    <>
      <PageHeader
        eyebrow="Settings"
        title="Settings"
        subtitle="Review managed storage roots and deployment defaults."
      />

      <section className="settingsGrid">
        <ManagedRootsPanel paths={paths} />
        <UserSkillsGitSettingsPanel
          status={status}
          userSkillsGit={userSkillsGit}
          onSave={onSaveUserSkillsRemote}
        />
        <StatusRefreshSettingsPanel
          preferences={preferences}
          status={status}
          onSave={onSaveStatusRefreshInterval}
        />
      </section>
    </>
  );
}

function StatusRefreshSettingsPanel({ preferences, status, onSave }) {
  const [intervalMinutes, setIntervalMinutes] = useState(
    String(preferences.statusRefreshIntervalMinutes || 5)
  );
  const [saveStatus, setSaveStatus] = useState('idle');
  const [message, setMessage] = useState('');

  useEffect(() => {
    setIntervalMinutes(String(preferences.statusRefreshIntervalMinutes || 5));
  }, [preferences.statusRefreshIntervalMinutes]);

  async function submit(event) {
    event.preventDefault();
    setSaveStatus('saving');
    setMessage('');

    try {
      await onSave(Number(intervalMinutes));
      setSaveStatus('saved');
      setMessage('Saved.');
    } catch (error) {
      setSaveStatus('error');
      setMessage(error.message || String(error) || 'Unable to save refresh interval.');
    }
  }

  return (
    <aside className="panel compactPanel">
      <div className="panelHeader compact">
        <div>
          <h2>Status refresh</h2>
          <p>Dashboard status checks run automatically.</p>
        </div>
      </div>
      <form className="settingsForm" onSubmit={submit}>
        <label className="remoteImportField">
          <span>Auto refresh interval</span>
          <div className="numberFieldRow">
            <input
              min="1"
              max="1440"
              step="1"
              type="number"
              value={intervalMinutes}
              onChange={(event) => {
                setIntervalMinutes(event.target.value);
                setMessage('');
              }}
            />
            <span>minutes</span>
          </div>
        </label>
        <div className="settingsActions">
          {message ? <span className={saveStatus === 'error' ? 'settingsError' : 'settingsSaved'}>{message}</span> : <span />}
          <button className="button primary" disabled={status === 'checking' || saveStatus === 'saving'} type="submit">
            {saveStatus === 'saving' ? 'Saving...' : 'Save interval'}
          </button>
        </div>
      </form>
    </aside>
  );
}

function UserSkillsGitSettingsPanel({ status, userSkillsGit, onSave }) {
  const [remoteUrl, setRemoteUrl] = useState(userSkillsGit.remoteUrl || '');
  const [saveStatus, setSaveStatus] = useState('idle');
  const [message, setMessage] = useState('');

  useEffect(() => {
    setRemoteUrl(userSkillsGit.remoteUrl || '');
  }, [userSkillsGit.remoteUrl]);

  async function submit(event) {
    event.preventDefault();
    setSaveStatus('saving');
    setMessage('');

    try {
      await onSave(remoteUrl);
      setSaveStatus('saved');
      setMessage('Saved.');
    } catch (error) {
      setSaveStatus('error');
      setMessage(error.message || String(error) || 'Unable to save remote URL.');
    }
  }

  return (
    <aside className="panel compactPanel">
      <div className="panelHeader compact">
        <div>
          <h2>User skills Git</h2>
          <p>Shared repository used by every local user skill.</p>
        </div>
      </div>
      <form className="settingsForm" onSubmit={submit}>
        <label className="remoteImportField">
          <span>Remote URL</span>
          <input
            placeholder="git@github.com:santosli/my-skills.git"
            value={remoteUrl}
            onChange={(event) => setRemoteUrl(event.target.value)}
          />
        </label>
        <PathList
          items={[
            ['Repository', userSkillsGit.repoPath || '~/SkillBox/user-skills'],
            ['Branch', userSkillsGit.branch || 'main'],
            ['State', userSyncLabel(userSkillsGit)]
          ]}
        />
        <div className="settingsActions">
          {message ? <span className={saveStatus === 'error' ? 'settingsError' : 'settingsSaved'}>{message}</span> : <span />}
          <button className="button primary" disabled={status === 'syncing' || saveStatus === 'saving'} type="submit">
            {saveStatus === 'saving' ? 'Saving...' : 'Save remote'}
          </button>
        </div>
      </form>
    </aside>
  );
}

function ManagedRootsPanel({ paths }) {
  return (
    <aside className="panel compactPanel">
      <div className="panelHeader compact">
        <div>
          <h2>Managed roots</h2>
          <p>Import will copy first, then replace runtime folders with symlinks.</p>
        </div>
      </div>
      <PathList
        items={[
          ['Managed root', paths?.root],
          ['User skills', paths?.userSkillsRoot],
          ['Remote skills', paths?.remoteSkillsRoot],
          ['Deploy mode', 'Copy, backup, symlink']
        ]}
      />
    </aside>
  );
}

function RemoteImportDialog({ error, mode, status, value, onClose, onModeChange, onSubmit, onValueChange }) {
  const isMarkdown = mode === 'markdown';

  return (
    <div className="modalBackdrop" role="presentation">
      <section className="remoteImportDialog" role="dialog" aria-modal="true" aria-labelledby="remote-import-title">
        <div className="importSheetHeader">
          <div>
            <h2 id="remote-import-title">Import skill</h2>
            <p>Provide a skill URL or a local Markdown file to review before importing.</p>
          </div>
          <button className="iconButton" type="button" aria-label="Close remote import" onClick={onClose}>
            x
          </button>
        </div>

        <form className="remoteImportForm" onSubmit={onSubmit}>
          <div className="remoteImportModes" role="group" aria-label="Import source type">
            <button
              className={mode === 'url' ? 'active' : ''}
              type="button"
              onClick={() => onModeChange('url')}
            >
              Skill URL
            </button>
            <button
              className={isMarkdown ? 'active' : ''}
              type="button"
              onClick={() => onModeChange('markdown')}
            >
              Markdown file
            </button>
          </div>

          <label className="remoteImportField">
            <span>{isMarkdown ? 'Markdown file path' : 'Skill URL'}</span>
            <input
              autoFocus
              placeholder={
                isMarkdown
                  ? '~/Downloads/SKILL.md'
                  : 'https://github.com/owner/repo/tree/main/path/to/skill'
              }
              type={isMarkdown ? 'text' : 'url'}
              value={value}
              onChange={(event) => onValueChange(event.target.value)}
            />
          </label>

          <p className="remoteImportHint">
            {isMarkdown
              ? 'Use a local .md file path. SkillBox will turn it into a reviewable import candidate.'
              : 'Use a GitHub tree, blob, raw, or API URL that points to a skill directory or SKILL.md.'}
          </p>
          {error ? <div className="formError">{error}</div> : null}

          <div className="remoteImportFooter">
            <button className="button secondary" type="button" onClick={onClose}>
              Cancel
            </button>
            <button className="button primary" disabled={status === 'importing'} type="submit">
              Review import
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}

function UserSkillsSyncDialog({
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
    <div className="modalBackdrop" role="presentation">
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
                placeholder="git@github.com:santosli/my-skills.git"
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

function GitDiffView({ diff }) {
  const rows = parseUnifiedDiff(diff);

  if (rows.length === 0) {
    return <div className="gitDiffEmpty">No diff to show.</div>;
  }

  return (
    <div className="githubDiffScroller">
      <table className="githubDiffTable" aria-label="Unified diff">
        <tbody>
          {rows.map((row, index) => (
            <tr className={`githubDiffRow ${row.kind}`} key={`${index}-${row.kind}`}>
              <td className="githubDiffLineNumber">{row.oldLine ?? ''}</td>
              <td className="githubDiffLineNumber">{row.newLine ?? ''}</td>
              <td className="githubDiffMarker">{row.marker}</td>
              <td className="githubDiffCode">{row.content || ' '}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function LocalImportConfirmationDialog({
  candidates,
  dontShowAgain,
  status,
  onClose,
  onConfirm,
  onDontShowAgainChange
}) {
  const shownCandidates = candidates.slice(0, 3);
  const remainingCount = Math.max(candidates.length - shownCandidates.length, 0);

  return (
    <div className="modalBackdrop" role="presentation">
      <section className="localImportDialog" role="dialog" aria-modal="true" aria-labelledby="local-import-title">
        <div className="importSheetHeader">
          <div>
            <h2 id="local-import-title">Confirm local import</h2>
            <p>SkillBox will move the selected skill folders into the managed store.</p>
          </div>
          <button className="iconButton" type="button" aria-label="Close local import confirmation" onClick={onClose}>
            x
          </button>
        </div>

        <div className="localImportBody">
          <div className="localImportImpact">
            <strong>{candidates.length} selected</strong>
            <p>
              The original folders will be replaced with symlinks to the managed copies, and the
              moved folders will be kept under the SkillBox import backups.
            </p>
          </div>

          <ul className="localImportPaths" aria-label="Selected local skill paths">
            {shownCandidates.map((candidate) => (
              <li key={candidate.sourcePath}>
                <span>{candidate.name}</span>
                <code>{compactPath(candidate.sourcePath)}</code>
              </li>
            ))}
            {remainingCount > 0 ? <li className="muted">+{remainingCount} more</li> : null}
          </ul>

          <label className="localImportPreference">
            <input
              checked={dontShowAgain}
              type="checkbox"
              onChange={(event) => onDontShowAgainChange(event.target.checked)}
            />
            <span>Don't show this again</span>
          </label>
        </div>

        <div className="localImportFooter">
          <button className="button secondary" disabled={status === 'importing'} type="button" onClick={onClose}>
            Cancel
          </button>
          <button className="button primary" disabled={status === 'importing'} type="button" onClick={onConfirm}>
            {status === 'importing' ? 'Importing...' : 'Confirm import'}
          </button>
        </div>
      </section>
    </div>
  );
}

function ImportReview({ candidates, onClose, onImport, onToggleAll, onToggleSelected, onTypeChange, status }) {
  const [isImportedExpanded, setIsImportedExpanded] = useState(false);
  const [isSystemExpanded, setIsSystemExpanded] = useState(false);
  const importedCandidates = candidates.filter((candidate) => candidate.importStatus === 'imported');
  const systemCandidates = candidates.filter((candidate) => candidate.importStatus === 'system');
  const reviewCandidates = candidates.filter(
    (candidate) => candidate.importStatus !== 'imported' && candidate.importStatus !== 'system'
  );
  const selectableCount = candidates.filter(isImportableCandidate).length;
  const selectedCount = candidates.filter((candidate) => candidate.isSelected && isImportableCandidate(candidate)).length;
  const isAllSelected = selectableCount > 0 && selectedCount === selectableCount;

  return (
    <div className="modalBackdrop" role="presentation">
      <section className="importSheet" role="dialog" aria-modal="true" aria-labelledby="import-review-title">
        <div className="importSheetHeader">
          <div>
            <h2 id="import-review-title">Import Review</h2>
            <p>Confirm each skill type before SkillBox copies it into the managed store.</p>
          </div>
          <button className="iconButton" type="button" aria-label="Close import review" onClick={onClose}>
            x
          </button>
        </div>

        <div className="candidateList">
          {reviewCandidates.map((candidate) => (
            <CandidateRow
              candidate={candidate}
              key={candidate.sourcePath}
              onToggleSelected={onToggleSelected}
              onTypeChange={onTypeChange}
            />
          ))}
          <CollapsedCandidateGroup
            candidates={systemCandidates}
            isExpanded={isSystemExpanded}
            label="System skills"
            onToggle={() => setIsSystemExpanded((current) => !current)}
            onToggleSelected={onToggleSelected}
            onTypeChange={onTypeChange}
          />
          <CollapsedCandidateGroup
            candidates={importedCandidates}
            isExpanded={isImportedExpanded}
            label="Imported skills"
            onToggle={() => setIsImportedExpanded((current) => !current)}
            onToggleSelected={onToggleSelected}
            onTypeChange={onTypeChange}
          />
        </div>

        <div className="importSheetFooter">
          <div className="importSelectionSummary">
            <button
              className="selectAllButton"
              disabled={selectableCount === 0 || status === 'importing'}
              type="button"
              onClick={onToggleAll}
            >
              {isAllSelected ? 'Unselect all' : 'Select all'}
            </button>
            <span>{selectedCount} selected</span>
          </div>
          <div className="headerActions">
            <button className="button secondary" type="button" onClick={onClose}>
              Cancel
            </button>
            <button
              className="button primary"
              disabled={status === 'importing' || selectedCount === 0}
              type="button"
              onClick={onImport}
            >
              {status === 'importing' ? 'Importing...' : 'Import selected'}
            </button>
          </div>
        </div>
      </section>
    </div>
  );
}

function CollapsedCandidateGroup({
  candidates,
  isExpanded,
  label,
  onToggle,
  onToggleSelected,
  onTypeChange
}) {
  if (candidates.length === 0) {
    return null;
  }

  return (
    <section className="collapsedCandidateGroup">
      <button
        className="collapsedCandidateToggle"
        type="button"
        aria-expanded={isExpanded}
        onClick={onToggle}
      >
        <span>
          {label}
          <strong>{candidates.length}</strong>
        </span>
        <span>{isExpanded ? 'Hide' : 'Show'}</span>
      </button>
      {isExpanded ? (
        <div className="collapsedCandidateRows">
          {candidates.map((candidate) => (
            <CandidateRow
              candidate={candidate}
              key={candidate.sourcePath}
              onToggleSelected={onToggleSelected}
              onTypeChange={onTypeChange}
            />
          ))}
        </div>
      ) : null}
    </section>
  );
}

function CandidateRow({ candidate, onToggleSelected, onTypeChange }) {
  return (
    <div className={candidateRowClass(candidate)}>
      <label className="candidateCheck">
        <input
          checked={candidate.isSelected}
          disabled={!isImportableCandidate(candidate)}
          type="checkbox"
          onChange={() => onToggleSelected(candidate)}
        />
        <span />
      </label>

      <div className="candidateMain">
        <div className="candidateTitle">
          <strong>{candidate.name}</strong>
          <SourceIcon candidate={candidate} />
          <Badge tone={candidate.skillType === 'user' ? 'green' : 'blue'}>
            {candidate.skillType === 'user' ? 'User skill' : 'Remote skill'}
          </Badge>
          {candidate.importStatus === 'system' ? <Badge tone="slate">System</Badge> : null}
          {candidate.importStatus === 'imported' ? <Badge tone="slate">Imported</Badge> : null}
          {candidate.conflict ? <Badge tone="red">Conflict</Badge> : null}
        </div>
        <small>{candidate.description || 'No description in SKILL.md'}</small>
        <code>{compactPath(candidate.sourcePath)}</code>
        {candidateStatusNote(candidate) ? <p>{candidateStatusNote(candidate)}</p> : null}
      </div>

      <div className="candidateTypeSwitch" role="group" aria-label={`${candidate.name} type`}>
        <button
          className={candidate.skillType === 'user' ? 'active' : ''}
          disabled={!isImportableCandidate(candidate)}
          type="button"
          onClick={() => onTypeChange(candidate, 'user')}
        >
          User
        </button>
        <button
          className={candidate.skillType === 'remote' ? 'active' : ''}
          disabled={!isImportableCandidate(candidate)}
          type="button"
          onClick={() => onTypeChange(candidate, 'remote')}
        >
          Remote
        </button>
      </div>
    </div>
  );
}

function SkillDetailDialog({
  skill,
  status,
  userSkillsGit,
  onCheckUpdates,
  onClose,
  onOpenSyncSetup,
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

  function closeOnBackdrop(event) {
    if (event.target === event.currentTarget) {
      onClose();
    }
  }

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
    <div className="modalBackdrop skillDetailBackdrop" role="presentation" onMouseDown={closeOnBackdrop}>
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
            <h2 id="skill-detail-title">{skill.name}</h2>
          </div>
          <button className="iconButton skillDetailCloseButton" type="button" aria-label="Close skill detail" onClick={onClose}>
            <X aria-hidden="true" />
          </button>
        </header>

        <p className="skillDetailDescription">
          {skill.description || 'No description in SKILL.md frontmatter.'}
        </p>

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
              placeholder="new tag"
              value={tagInput}
              onChange={(event) => setTagInput(event.target.value)}
            />
            <button disabled={!pendingTag} type="submit">
              Add
            </button>
          </div>
        </form>

        <div className="skillDetailAgentRow">
          <span>Installed agents</span>
          <AgentIconStack agents={skill.installedAgents} />
        </div>

        <footer className="skillDetailActions">
          <button
            aria-pressed={skill.isFavorite}
            className={skill.isFavorite ? 'detailFavoriteButton active' : 'detailFavoriteButton'}
            type="button"
            onClick={() => onToggleFavorite(skill.name)}
          >
            <Star aria-hidden="true" />
            {skill.isFavorite ? 'Favorited' : 'Favorite'}
          </button>

          {skill.type === 'user' ? (
            <button
              className="button primary"
              disabled={isSyncing}
              type="button"
              onClick={onOpenSyncSetup}
            >
              {isPreparingSync ? 'Preparing...' : status === 'syncing' ? 'Syncing...' : syncAction}
            </button>
          ) : (
            <button
              className="button primary"
              disabled={isChecking}
              type="button"
              onClick={() => onCheckUpdates()}
            >
              {isChecking ? 'Checking...' : 'Check update'}
            </button>
          )}
        </footer>
      </section>
    </div>
  );
}

function PageHeader({ actions, eyebrow, subtitle, title }) {
  return (
    <header className="pageHeader">
      <div>
        <p className="eyebrow">{eyebrow}</p>
        <h1>{title}</h1>
        <p>{subtitle}</p>
      </div>
      {actions ? <div className="headerActions">{actions}</div> : null}
    </header>
  );
}

function NavButton({ active, icon, label, onClick }) {
  return (
    <button className={active ? 'navButton active' : 'navButton'} type="button" onClick={onClick}>
      <span className="navIcon">
        <Icon name={icon} />
      </span>
      {label}
    </button>
  );
}

function FooterButton({ active = false, icon, label, onClick }) {
  return (
    <button className={active ? 'active' : ''} type="button" onClick={onClick}>
      <Icon name={icon} />
      {label}
    </button>
  );
}

function SourceIcon({ candidate }) {
  const source = candidateSource(candidate);
  if (!source) {
    return null;
  }

  const iconSource = source.kind === 'agent' ? codexCliIcon : codexAppIcon;

  return (
    <span className={`sourceIcon ${source.kind}`} title={source.label} aria-label={source.label}>
      <img src={iconSource} alt="" aria-hidden="true" />
    </span>
  );
}

function Icon({ name }) {
  if (name === 'setup') {
    return (
      <svg aria-hidden="true" viewBox="0 0 24 24">
        <path d="M5 5h14v14H5z" />
        <path d="m9 12 2 2 4-5" />
      </svg>
    );
  }

  if (name === 'dashboard') {
    return (
      <svg aria-hidden="true" viewBox="0 0 24 24">
        <path d="M4 5h7v7H4z" />
        <path d="M13 5h7v4h-7z" />
        <path d="M13 11h7v8h-7z" />
        <path d="M4 14h7v5H4z" />
      </svg>
    );
  }

  if (name === 'user-skills') {
    return (
      <svg aria-hidden="true" viewBox="0 0 24 24">
        <path d="M16 19a4 4 0 0 0-8 0" />
        <circle cx="12" cy="8" r="3" />
        <path d="M4 5h2" />
        <path d="M18 5h2" />
        <path d="M4 12h2" />
        <path d="M18 12h2" />
      </svg>
    );
  }

  if (name === 'remote-skills') {
    return (
      <svg aria-hidden="true" viewBox="0 0 24 24">
        <circle cx="12" cy="12" r="8" />
        <path d="M4 12h16" />
        <path d="M12 4a12 12 0 0 1 0 16" />
        <path d="M12 4a12 12 0 0 0 0 16" />
      </svg>
    );
  }

  if (name === 'settings') {
    return (
      <svg aria-hidden="true" viewBox="0 0 24 24">
        <circle cx="12" cy="12" r="3" />
        <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 0 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6V21a2 2 0 0 1-4 0v-.1a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1a2 2 0 0 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.6-1H3a2 2 0 0 1 0-4h.1a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9l-.1-.1a2 2 0 0 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.9.3h.1a1.7 1.7 0 0 0 1-1.6V3a2 2 0 0 1 4 0v.1a1.7 1.7 0 0 0 1 1.6h.1a1.7 1.7 0 0 0 1.9-.3l.1-.1a2 2 0 0 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.9v.1a1.7 1.7 0 0 0 1.6 1H21a2 2 0 0 1 0 4h-.1a1.7 1.7 0 0 0-1.5.8Z" />
      </svg>
    );
  }

  return (
    <svg aria-hidden="true" viewBox="0 0 24 24">
      <circle cx="12" cy="12" r="9" />
      <path d="M9.8 9a2.4 2.4 0 0 1 4.5 1.2c0 1.7-2.1 2-2.1 3.5" />
      <path d="M12 17h.01" />
    </svg>
  );
}

function Badge({ children, tone = 'slate' }) {
  return <span className={`badge ${tone}`}>{children}</span>;
}

function PathList({ items }) {
  return (
    <dl className="pathList">
      {items.map(([label, value]) => (
        <div key={label}>
          <dt>{label}</dt>
          <dd>{value || 'Not available'}</dd>
        </div>
      ))}
    </dl>
  );
}

function normalizeSkill(skill) {
  const sourceRoot = skill.sourceRoot || skill.source_root;
  const isSymlink = skill.isSymlink || skill.is_symlink;
  const type = skill.type || inferType(sourceRoot);

  return {
    ...skill,
    sourceRoot,
    contentHash: skill.contentHash || skill.content_hash,
    skillMdPath: skill.skillMdPath || skill.skill_md_path,
    isSymlink,
    type,
    status: skill.status || defaultSkillStatus(type)
  };
}

function normalizePreferences(preferences) {
  return {
    skipLocalImportConfirmation: Boolean(
      preferences?.skipLocalImportConfirmation ?? preferences?.skip_local_import_confirmation
    ),
    statusRefreshIntervalMinutes: normalizeStatusRefreshIntervalMinutes(
      preferences?.statusRefreshIntervalMinutes ?? preferences?.status_refresh_interval_minutes
    )
  };
}

function readPreviewPreferences() {
  try {
    const statusRefreshIntervalMinutes = window.localStorage.getItem(
      previewStatusRefreshIntervalStorageKey
    );

    return {
      skipLocalImportConfirmation: window.localStorage.getItem(previewPreferenceStorageKey) === 'true',
      statusRefreshIntervalMinutes: normalizeStatusRefreshIntervalMinutes(
        statusRefreshIntervalMinutes
      )
    };
  } catch {
    return normalizePreferences(null);
  }
}

function readDashboardFavorites() {
  try {
    return normalizeFavoriteNames(window.localStorage.getItem(dashboardFavoriteStorageKey));
  } catch {
    return [];
  }
}

function readDashboardTagOverrides() {
  try {
    return normalizeDashboardTagOverrides(window.localStorage.getItem(dashboardTagStorageKey));
  } catch {
    return {};
  }
}

function remoteImportCandidate(mode, value) {
  const name = inferSkillNameFromImportValue(value);
  const isMarkdown = mode === 'markdown';

  return {
    name,
    description: isMarkdown ? 'Remote skill created from a Markdown file.' : 'Remote skill source provided by URL.',
    sourcePath: value,
    sourceRoot: inferImportSourceRoot(value),
    contentHash: previewContentHash(value),
    suggestedType: 'remote',
    skillType: 'remote',
    suggestionReason: isMarkdown ? 'User provided Markdown file' : 'User provided skill URL',
    importOrigin: 'remote-input',
    importStatus: 'importable',
    isSelected: true,
    conflict: null
  };
}

function applyPreviewImportStatuses(candidates, importedSkills) {
  const importedHashes = new Set(importedSkills.map((skill) => skill.contentHash).filter(Boolean));
  const importedNames = new Set(importedSkills.map((skill) => skill.name).filter(Boolean));

  return candidates.map((candidate) => {
    if (candidate.importStatus !== 'importable') {
      return candidate;
    }

    if (!importedHashes.has(candidate.contentHash) && !importedNames.has(candidate.name)) {
      return candidate;
    }

    return {
      ...candidate,
      importStatus: 'imported',
      isSelected: false,
      suggestionReason: 'Imported; source links to SkillBox'
    };
  });
}

function shouldConfirmLocalImport(candidates, preferences) {
  if (preferences.skipLocalImportConfirmation) {
    return false;
  }

  return candidates.some((candidate) => isImportableCandidate(candidate) && requiresLocalImportConfirmation(candidate));
}

function requiresLocalImportConfirmation(candidate) {
  const sourcePath = String(candidate.sourcePath || '');

  if (candidate.importOrigin === 'remote-input') {
    return false;
  }

  if (isHttpUrl(sourcePath) || sourcePath.toLowerCase().endsWith('.md')) {
    return false;
  }

  return true;
}

function importNotice(prefix, message) {
  return [prefix, message].filter(Boolean).join(' ');
}

function isImportableCandidate(candidate) {
  return candidate.importStatus === 'importable' && !candidate.conflict;
}

function candidateRowClass(candidate) {
  return [
    'candidateRow',
    candidate.conflict ? 'conflict' : '',
    candidate.importStatus === 'imported' ? 'imported' : '',
    candidate.importStatus === 'system' ? 'system' : ''
  ]
    .filter(Boolean)
    .join(' ');
}

function candidateStatusNote(candidate) {
  if (candidate.conflict) {
    return candidate.conflict;
  }
  if (candidate.importStatus === 'imported' || candidate.importStatus === 'system') {
    return '';
  }
  if (candidateSource(candidate)) {
    return '';
  }
  return candidate.suggestionReason;
}

function candidateSource(candidate) {
  const values = [
    candidate.sourceRoot,
    candidate.sourcePath,
    candidate.realPath,
    candidate.suggestionReason
  ]
    .filter(Boolean)
    .map((value) => String(value));
  const combined = values.join(' ');

  if (combined.includes('/.agents/skills') || combined.includes('~/.agents/skills')) {
    return { kind: 'agent', label: 'From ~/.agents/skills' };
  }

  if (combined.includes('/.codex/skills') || combined.includes('~/.codex/skills')) {
    return { kind: 'codex', label: 'From ~/.codex/skills' };
  }

  return null;
}

function toggleImportCandidateSelection(candidates) {
  const selectable = candidates.filter(isImportableCandidate);
  const shouldSelectAll = selectable.some((candidate) => !candidate.isSelected);

  return candidates.map((candidate) =>
    isImportableCandidate(candidate) ? { ...candidate, isSelected: shouldSelectAll } : candidate
  );
}

function isHttpUrl(value) {
  try {
    const parsed = new URL(value);
    return parsed.protocol === 'http:' || parsed.protocol === 'https:';
  } catch {
    return false;
  }
}

function inferSkillNameFromImportValue(value) {
  const clean = value.split(/[?#]/)[0].replace(/\/+$/, '');
  const parts = clean.split(/[\\/]/).filter(Boolean);
  let name = parts[parts.length - 1] || 'remote-skill';

  if (name.toLowerCase() === 'skill.md' && parts.length > 1) {
    name = parts[parts.length - 2];
  } else if (name.toLowerCase().endsWith('.md')) {
    name = name.slice(0, -3);
  }

  return name || 'remote-skill';
}

function inferImportSourceRoot(value) {
  try {
    const parsed = new URL(value);
    const pathParts = parsed.pathname.split('/').filter(Boolean).slice(0, 2);
    return [parsed.hostname, ...pathParts].join('/');
  } catch {
    const clean = value.split(/[?#]/)[0].replace(/\/+$/, '');
    const parts = clean.split(/[\\/]/).filter(Boolean);
    return parts.slice(0, -1).join('/') || clean;
  }
}

function previewContentHash(value) {
  let hash = 0;
  for (const char of value) {
    hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  }
  return `preview-${hash.toString(16).padStart(8, '0')}`;
}

function candidateToPreviewSkill(candidate) {
  const type = candidate.skillType || candidate.suggestedType || 'user';
  const managedPath =
    type === 'user'
      ? joinPath(previewPaths.userSkillsRoot, candidate.name)
      : joinPath(previewPaths.remoteSkillsRoot, `${candidate.name}/current`);

  return {
    name: candidate.name,
    type,
    description: candidate.description,
    sourceRoot: candidate.sourceRoot,
    path: managedPath,
    skillMdPath: joinPath(managedPath, 'SKILL.md'),
    status: defaultSkillStatus(type),
    isSymlink: true,
    contentHash: candidate.contentHash
  };
}

function mergeSkills(current, imported) {
  const next = new Map(current.map((skill) => [skill.name, skill]));
  for (const skill of imported) {
    next.set(skill.name, skill);
  }
  return Array.from(next.values()).sort((left, right) => left.name.localeCompare(right.name));
}

function normalizePaths(paths) {
  if (!paths) return paths;

  return {
    ...paths,
    userSkillsRoot: paths.userSkillsRoot || paths.user_skills_root,
    remoteSkillsRoot: paths.remoteSkillsRoot || paths.remote_skills_root,
    databasePath: paths.databasePath || paths.database_path
  };
}

function inferType(sourceRoot = '') {
  if (String(sourceRoot).includes('.agents')) return 'user';
  return 'remote';
}

function defaultSkillStatus(type) {
  return type === 'user' ? 'sync not checked' : 'update not checked';
}

function hasAvailableUpdate(skill) {
  const normalized = String(skill.status || '').toLowerCase();
  return skill.type === 'remote' && (normalized.includes('update available') || normalized.includes('new version'));
}

function labelize(value = '') {
  return String(value).replace(/[-_]/g, ' ');
}

function compactPath(value = '') {
  return String(value || 'Not available').replace('/Users/santos', '~');
}

function joinPath(root, child) {
  if (!root) return child;
  return `${String(root).replace(/\/$/, '')}/${child}`;
}
