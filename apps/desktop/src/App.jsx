import React, { useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  AlertTriangle,
  Check,
  ExternalLink,
  FolderCode,
  FolderOpen,
  Gauge,
  Grid3X3,
  History as HistoryIcon,
  Import as ImportIcon,
  Link2,
  List,
  MessageCircleQuestionMark,
  PackagePlus,
  Plus,
  RefreshCw,
  Search,
  Settings2,
  Star,
  Trash2,
  Unlink,
  X
} from 'lucide-react';
import desktopPackage from '../package.json';
import skillBoxAppIcon from '../src-tauri/icons/icon.png';
import claudeCodeIcon from './assets/claude-code-icon.svg';
import codexAppIcon from './assets/codex-app-icon.png';
import codexCliIcon from './assets/codex-cli-icon.png';
import { dashboardTabItems, skillMatchesDashboardFilters, sortDashboardSkills } from './dashboardFilters.js';
import {
  dashboardFilterOptions,
  deriveDashboardSkill,
  normalizeDashboardTagOverrides,
  normalizeEditableTags,
  normalizeFavoriteNames
} from './dashboardMetadata.js';
import { GitDiffView } from './GitDiffView.jsx';
import { normalizeImportCandidate } from './importCandidates.js';
import { closeOnBackdropClick } from './modalEvents.js';
import {
  canApplyRemoteVersionChange,
  formatOperationTimestamp,
  formatRemoteRefBehavior,
  normalizeRemoteSourceCandidates,
  normalizeRemoteSourceBindingPreview,
  normalizeRemoteVersionPreview,
  remoteSkillUpdateVersionLabel,
  remoteVersionActionLabel,
  shouldShowRemoteUpdateSummary
} from './remoteSkills.js';
import {
  dashboardStatusNotice,
  formatStatusCheckedAt,
  formatStatusNoticeCountdown,
  normalizeRemoteSkillUpdates,
  normalizeRemoteUpdateTimeoutSeconds,
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
import {
  normalizeWorkspace,
  normalizeWorkspaces,
  sidebarFooterItems,
  sidebarItems,
  workspaceCardMetaLabels,
  workspaceCounts,
  workspaceDeployChangeCount,
  workspaceDeploymentChanges,
  workspaceDeployPickerRows,
  workspaceDeployRequiresConfirmation,
  workspaceMatchesTypeFilter,
  workspaceSkillReviewMeta,
  workspaceTypeTabs
} from './workspaces.js';

const previewPaths = {
  root: '~/.skillbox',
  userSkillsRoot: '~/.skillbox/user-skills',
  remoteSkillsRoot: '~/.skillbox/remote-skills',
  databasePath: '~/.skillbox/skillbox.sqlite'
};

const previewPreferenceStorageKey = 'skillbox.skipLocalImportConfirmation';
const previewStatusRefreshIntervalStorageKey = 'skillbox.statusRefreshIntervalMinutes';
const previewRemoteUpdateTimeoutStorageKey = 'skillbox.remoteUpdateTimeoutSeconds';
const dashboardFavoriteStorageKey = 'skillbox.dashboardFavorites';
const dashboardTagStorageKey = 'skillbox.dashboardTags';
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

const previewWorkspaces = [
  {
    canonical_path: '/Users/santos/.codex/skills',
    path: '/Users/santos/.codex/skills',
    kind: 'global',
    source: 'auto',
    agent_id: 'codex',
    display_name: 'Codex',
    skill_count: 4,
    imported_skill_count: 2,
    last_scan_error_count: 0,
    last_scanned_at: '2026-05-26 08:00:00'
  },
  {
    canonical_path:
      '/Users/santos/Library/Mobile Documents/iCloud~md~obsidian/Documents/Pandora/.agents/skills',
    path:
      '/Users/santos/Library/Mobile Documents/iCloud~md~obsidian/Documents/Pandora/.agents/skills',
    kind: 'user',
    source: 'manual',
    agent_id: 'agents',
    display_name: 'Pandora',
    skill_count: 2,
    imported_skill_count: 1,
    last_scan_error_count: 0,
    last_scanned_at: '2026-05-26 08:00:00'
  }
];

const previewUsageHooks = [
  {
    target: 'codex_app',
    label: 'Codex App',
    configPath: '~/.codex/hooks.json',
    command: '~/.skillbox/bin/skillbox-usage-hook codex',
    installed: false,
    sharedConfigKey: 'codex'
  },
  {
    target: 'codex_cli',
    label: 'Codex CLI',
    configPath: '~/.codex/hooks.json',
    command: '~/.skillbox/bin/skillbox-usage-hook codex',
    installed: false,
    sharedConfigKey: 'codex'
  },
  {
    target: 'claude_code_cli',
    label: 'Claude Code CLI',
    configPath: '~/.claude/settings.json',
    command: '~/.skillbox/bin/skillbox-usage-hook claude-code',
    installed: false,
    sharedConfigKey: 'claude-code'
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

function previewHistory() {
  const now = Date.now();
  return {
    skill_usage_count: 3,
    operation_count: 2,
    entries: [
      {
        id: 'preview-usage-grill-me',
        kind: 'skill_usage',
        timestamp: new Date(now - 12 * 60 * 1000).toISOString(),
        title: 'Skill call: grill-me',
        subtitle: 'codex in ~/.skillbox/remote-skills/grill-me/versions',
        skill_name: 'grill-me',
        agent_id: 'codex',
        runtime_root: '~/.skillbox/remote-skills/grill-me/versions',
        prompt_excerpt: 'Use grill-me to review the skill usage stats plan'
      },
      {
        id: 'preview-operation-install',
        kind: 'operation',
        timestamp: Math.floor((now - 42 * 60 * 1000) / 1000).toString(),
        title: 'Installed find-skills',
        subtitle: 'install_remote_skill by desktop',
        status: 'succeeded',
        operation_type: 'install_remote_skill',
        actor: 'desktop',
        entity_type: 'skill',
        entity_name: 'find-skills'
      },
      {
        id: 'preview-usage-frontend',
        kind: 'skill_usage',
        timestamp: new Date(now - 2 * 60 * 60 * 1000).toISOString(),
        title: 'Skill call: frontend-design',
        subtitle: 'codex in ~/.skillbox/remote-skills/frontend-design/versions',
        skill_name: 'frontend-design',
        agent_id: 'codex',
        runtime_root: '~/.skillbox/remote-skills/frontend-design/versions',
        prompt_excerpt: 'Make the History timeline easier to scan'
      }
    ]
  };
}

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
  }, [page, filter, workspaceTypeFilter]);

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
      const hookRows = await invoke('usage_hook_statuses');
      setUsageHooks(normalizeUsageHookStatuses(hookRows));
      setNotice('Usage hook injection updated.');
      setStatus('ready');
    } catch (hookError) {
      setError(hookError.message || String(hookError) || 'Unable to install usage hook.');
      setStatus('ready');
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
              source_url: `https://github.com/santos/skillbox-preview/tree/main/remote-skills/${skillName}`,
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
            owner: 'santos',
            repo: 'skillbox-preview',
            path: `remote-skills/${skillName}`,
            reference: 'main',
            source_url: `https://github.com/santos/skillbox-preview/tree/main/remote-skills/${skillName}`,
            repo_url: 'https://github.com/santos/skillbox-preview.git',
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
            <span>Local skill manager</span>
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
              onClick={item.id === 'settings' ? () => setPage('settings') : undefined}
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
            paths={paths}
            preferences={preferences}
            status={status}
            usageHooks={usageHooks}
            userSkillsGit={userSkillsGit}
            onOpenUsageHookConfig={openUsageHookConfig}
            onInstallUsageHook={installUsageHook}
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
  const [previewAction, setPreviewAction] = useState(null);
  const previewIndex = { refresh: 0, import: 1, install: 2 }[previewAction] || 0;

  const actions = [
    {
      id: 'refresh',
      icon: RefreshCw,
      label: isChecking ? 'Refreshing' : 'Refresh',
      loading: isChecking,
      disabled: isChecking,
      onClick: onRefreshStatuses
    },
    {
      id: 'import',
      icon: ImportIcon,
      label: 'Import',
      onClick: onRefresh
    },
    {
      id: 'install',
      icon: PackagePlus,
      label: 'Install',
      onClick: onInstall
    }
  ];

  return (
    <div
      className={previewAction ? 'dashboardActionGroup previewing' : 'dashboardActionGroup'}
      aria-label="Skill actions"
      style={{ '--dashboard-action-index': previewIndex }}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) {
          setPreviewAction(null);
        }
      }}
      onMouseLeave={() => setPreviewAction(null)}
    >
      <span className="dashboardActionIndicator" aria-hidden="true" />
      {actions.map((action) => {
        const Icon = action.icon;
        const actionClassName = action.loading
          ? 'dashboardActionButton loading'
          : previewAction === action.id
            ? 'dashboardActionButton preview'
            : 'dashboardActionButton';
        return (
          <button
            aria-busy={action.loading ? 'true' : undefined}
            className={actionClassName}
            disabled={action.disabled}
            key={action.id}
            type="button"
            onFocus={() => setPreviewAction(action.id)}
            onMouseEnter={() => setPreviewAction(action.id)}
            onClick={() => {
              action.onClick();
              setPreviewAction(null);
            }}
          >
            <Icon aria-hidden="true" />
            {action.label}
          </button>
        );
      })}
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
          <span className="skillCardTitleText">
            <strong>{skill.name}</strong>
            <span className="skillCardUsage">{skill.usageCount || 0} calls</span>
          </span>
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
      <span className="skillCardHeaderActions">
        <Badge tone={skill.statusTone}>{skill.statusLabel}</Badge>
        <button
          aria-label={skill.isFavorite ? `Remove ${skill.name} from favorites` : `Add ${skill.name} to favorites`}
          aria-pressed={skill.isFavorite}
          className={skill.isFavorite ? 'skillFavoriteButton active' : 'skillFavoriteButton'}
          type="button"
          onClick={() => onToggleFavorite(skill.name)}
        >
          <Star aria-hidden="true" />
        </button>
      </span>
    </article>
  );
}

function AgentIconStack({ agents = [], emptyLabel = 'No installed agent target', labelPrefix = 'Installed agents' }) {
  const visibleAgents = agents.slice(0, 4);
  const overflowCount = Math.max(agents.length - visibleAgents.length, 0);
  const overflowLabel = overflowCount
    ? agents
        .slice(visibleAgents.length)
        .map((agent) => agent.label)
        .join(', ')
    : '';
  const label = agents.length
    ? `${labelPrefix}: ${agents.map((agent) => agent.label).join(', ')}`
    : emptyLabel;

  return (
    <span className="skillAgentIcons" aria-label={label}>
      {visibleAgents.map((agent) => (
        <AgentIconBadge agent={agent} key={agent.id} />
      ))}
      {overflowCount > 0 ? (
        <span className="skillAgentIcon overflow" data-tooltip={overflowLabel} aria-label={overflowLabel}>
          +{overflowCount}
        </span>
      ) : null}
    </span>
  );
}

function AgentIconBadge({ agent }) {
  if (!agent) {
    return null;
  }

  const iconClass = agentIconClass(agent);
  const iconSource = agentIconSource(agent, iconClass);

  return (
    <span className={`skillAgentIcon ${iconClass}`} data-tooltip={agent.label} aria-label={agent.label}>
      {iconSource ? (
        <img src={iconSource} alt="" aria-hidden="true" />
      ) : (
        <span aria-hidden="true">{agent.iconLabel || agentInitial(agent)}</span>
      )}
    </span>
  );
}

function agentIconClass(agent) {
  return agent.iconClass || agent.id || 'local';
}

function agentIconSource(agent, iconClass = '') {
  if (agent.iconAsset === 'codex-app' || iconClass === 'codex-app' || agent.id === 'codex') {
    return codexAppIcon;
  }
  if (agent.iconAsset === 'codex-cli' || iconClass === 'codex-cli' || agent.id === 'agents') {
    return codexCliIcon;
  }
  if (agent.iconAsset === 'claude-code' || iconClass === 'claude-code' || agent.id === 'claude-code') {
    return claudeCodeIcon;
  }
  return null;
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

function WorkspacePage({
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

function HistoryPage({ error, filter, history, status, onFilter, onRefresh }) {
  const entries = history.entries || [];
  const tabs = [
    {
      id: 'all',
      label: 'All',
      count: numberOrZero(history.skillUsageCount) + numberOrZero(history.operationCount)
    },
    { id: 'skill_usage', label: 'Skill calls', count: numberOrZero(history.skillUsageCount) },
    { id: 'operation', label: 'Operations', count: numberOrZero(history.operationCount) }
  ];
  const filteredEntries =
    filter === 'all' ? entries : entries.filter((entry) => entry.kind === filter);
  const groupedEntries = groupHistoryEntriesByDay(filteredEntries);
  const isLoading = status === 'loading_history';

  return (
    <section className="dashboardFrame historyFrame" aria-label="History">
      {error ? <div className="notice">{error}</div> : null}
      <div className="dashboardTitleRow">
        <div className="dashboardTitleGroup">
          <h1>History</h1>
          <span className="dashboardCountPill">{filteredEntries.length}</span>
        </div>
      </div>

      <div className="dashboardControlRow historyControlRow">
        <div className="dashboardTypeTabs historyTypeTabs" role="tablist" aria-label="History type">
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
        <button className="button secondary" disabled={isLoading} type="button" onClick={onRefresh}>
          <RefreshCw aria-hidden="true" />
          {isLoading ? 'Loading...' : 'Refresh'}
        </button>
      </div>

      {filteredEntries.length > 0 ? (
        <div className="historyTimeline" aria-label="History entries">
          {groupedEntries.map((group) => (
            <section className="historyDayBlock" key={group.key} aria-label={`${group.label} history`}>
              <div className="historyDayHeader">
                <h2>{group.label}</h2>
                <span>{group.entries.length}</span>
              </div>
              <div className="historyDayRows">
                {group.entries.map((entry) => (
                  <HistoryRow entry={entry} key={`${entry.kind}:${entry.id}`} />
                ))}
              </div>
            </section>
          ))}
        </div>
      ) : (
        <div className="emptyState dashboardEmptyState historyEmptyState">
          <strong>No history yet</strong>
          <span>Skill calls and SkillBox operations will appear here.</span>
        </div>
      )}
    </section>
  );
}

function HistoryRow({ entry }) {
  const timestamp = formatOperationTimestamp(entry.timestamp);
  const timestampParts = timestamp.split(' ');
  const timestampTime = timestampParts.length > 1 ? timestampParts.slice(1).join(' ') : timestamp;
  const isUsage = entry.kind === 'skill_usage';
  const badgeLabel = isUsage ? 'Call' : entry.status || 'operation';
  const badgeTone = isUsage ? 'blue' : operationStatusTone(entry.status);
  const details = isUsage
    ? [entry.agentId, compactPath(entry.runtimeRoot)].filter(Boolean)
    : [entry.operationType, entry.actor, entry.entityName].filter(Boolean);
  const rowSubtitle = historyRowSubtitle(entry, isUsage);

  return (
    <article className={isUsage ? 'historyRow usage' : 'historyRow operation'}>
      <div className="historyRowTimeRail">
        {timestamp ? (
          <time className="historyRowTimestamp" dateTime={entry.timestamp}>
            <strong>{timestampTime}</strong>
          </time>
        ) : null}
      </div>
      <div className="historyRowTitle">
        <strong>{entry.title || entry.skillName || entry.operationType || 'History event'}</strong>
        <Badge tone={badgeTone}>{badgeLabel}</Badge>
      </div>
      <div className="historyRowMain">
        <div className="historyRowMeta">
          {details.map((detail) => (
            <span key={detail}>{detail}</span>
          ))}
        </div>
        {rowSubtitle ? <p>{rowSubtitle}</p> : null}
        {isUsage && entry.promptExcerpt ? (
          <div className="historyRowPrompt">
            <span>Prompt</span>
            <p>{entry.promptExcerpt}</p>
          </div>
        ) : null}
        {entry.error ? <small>{entry.error}</small> : null}
      </div>
    </article>
  );
}

function historyRowSubtitle(entry, isUsage) {
  if (isUsage) return '';

  const subtitle = String(entry.subtitle || '').trim();
  if (!subtitle) return '';

  const defaultOperationSubtitle = entry.operationType && entry.actor
    ? `${entry.operationType} by ${entry.actor}`
    : '';
  return subtitle === defaultOperationSubtitle ? '' : subtitle;
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

function WorkspaceAddDialog({ dialog, status, onClose, onSubmit, onUpdate }) {
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

function DeployWorkspaceDialog({
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

function SettingsPage({
  paths,
  preferences,
  status,
  usageHooks,
  userSkillsGit,
  onInstallUsageHook,
  onOpenUsageHookConfig,
  onSaveRemoteUpdateTimeout,
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
          onSaveRemoteUpdateTimeout={onSaveRemoteUpdateTimeout}
          onSave={onSaveStatusRefreshInterval}
        />
        <UsageHookSettingsPanel
          status={status}
          usageHooks={usageHooks}
          onInstall={onInstallUsageHook}
          onOpenConfig={onOpenUsageHookConfig}
        />
      </section>
    </>
  );
}

function UsageHookSettingsPanel({ status, usageHooks, onInstall, onOpenConfig }) {
  const hookGroups = groupUsageHooksByConfig(normalizeUsageHookStatuses(usageHooks));
  const isInstalling = status === 'installing_usage_hook';

  return (
    <aside className="panel compactPanel usageHookSettingsPanel">
      <div className="panelHeader compact">
        <div>
          <h2>Usage hook injection</h2>
          <p>Record real agent skill calls from runtime hooks.</p>
        </div>
      </div>
      <div className="usageHookList">
        {hookGroups.map((group) => (
          <div className="usageHookRow" key={group.key}>
            <div className="usageHookMain">
              <strong>{group.label}</strong>
              <small>{group.configPath || 'Config path unavailable'}</small>
              <code>{group.command || '~/.skillbox/bin/skillbox-usage-hook'}</code>
            </div>
            <div className="usageHookActions">
              <Badge tone={usageHookBadgeTone(group)}>
                {usageHookStatusLabel(group)}
              </Badge>
              <button
                className="button secondary"
                disabled={isInstalling || (group.installed && !group.configPath)}
                type="button"
                onClick={() =>
                  group.installed ? onOpenConfig(group.configPath) : onInstall(group.target)
                }
              >
                {isInstalling ? 'Injecting...' : group.installed ? 'Open' : 'Inject'}
              </button>
            </div>
            {group.activationNote ? (
              <small className="usageHookTrustNote">{group.activationNote}</small>
            ) : null}
          </div>
        ))}
      </div>
    </aside>
  );
}

function StatusRefreshSettingsPanel({ preferences, status, onSave, onSaveRemoteUpdateTimeout }) {
  const [intervalMinutes, setIntervalMinutes] = useState(
    String(preferences.statusRefreshIntervalMinutes || 5)
  );
  const [timeoutSeconds, setTimeoutSeconds] = useState(
    String(preferences.remoteUpdateTimeoutSeconds || 30)
  );
  const [saveStatus, setSaveStatus] = useState('idle');
  const [message, setMessage] = useState('');

  useEffect(() => {
    setIntervalMinutes(String(preferences.statusRefreshIntervalMinutes || 5));
    setTimeoutSeconds(String(preferences.remoteUpdateTimeoutSeconds || 30));
  }, [preferences.statusRefreshIntervalMinutes, preferences.remoteUpdateTimeoutSeconds]);

  async function submit(event) {
    event.preventDefault();
    setSaveStatus('saving');
    setMessage('');

    try {
      await onSave(Number(intervalMinutes));
      await onSaveRemoteUpdateTimeout(Number(timeoutSeconds));
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
        <label className="remoteImportField">
          <span>Git check timeout</span>
          <div className="numberFieldRow">
            <input
              min="5"
              max="300"
              step="1"
              type="number"
              value={timeoutSeconds}
              onChange={(event) => {
                setTimeoutSeconds(event.target.value);
                setMessage('');
              }}
            />
            <span>seconds</span>
          </div>
        </label>
        <div className="settingsActions">
          {message ? <span className={saveStatus === 'error' ? 'settingsError' : 'settingsSaved'}>{message}</span> : <span />}
          <button className="button primary" disabled={status === 'checking' || saveStatus === 'saving'} type="submit">
            {saveStatus === 'saving' ? 'Saving...' : 'Save status settings'}
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
            ['Repository', userSkillsGit.repoPath || '~/.skillbox/user-skills'],
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
    <div
      className="modalBackdrop"
      role="presentation"
      onMouseDown={(event) => closeOnBackdropClick(event, onClose)}
    >
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
    <div
      className="modalBackdrop"
      role="presentation"
      onMouseDown={(event) => closeOnBackdropClick(event, onClose)}
    >
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

function LoadingNotice({ children, compact = false }) {
  return (
    <div className={compact ? 'loadingNotice compact' : 'loadingNotice'} aria-live="polite">
      <span className="inlineSpinner" aria-hidden="true" />
      <span>{children}</span>
    </div>
  );
}

function RemoteSourceBindingDialog({
  dialog,
  onBind,
  onBindCandidate,
  onClose,
  onSearch,
  onUpdate,
  onViewCandidate
}) {
  const hasCandidates = dialog.candidates.length > 0;

  return (
    <div
      className="modalBackdrop"
      role="presentation"
      onMouseDown={(event) => closeOnBackdropClick(event, onClose)}
    >
      <section className="remoteImportDialog" role="dialog" aria-modal="true" aria-labelledby="remote-source-title">
        <div className="importSheetHeader">
          <div>
            <h2 id="remote-source-title">Bind source</h2>
            <p>Link a GitHub source without replacing the current version.</p>
          </div>
          <button className="iconButton" disabled={dialog.loading} type="button" aria-label="Close source binding" onClick={onClose}>
            <X aria-hidden="true" />
          </button>
        </div>
        <form className="remoteImportForm" onSubmit={onBind}>
          <label className="remoteImportField">
            <span>GitHub source URL</span>
            <input
              autoFocus
              disabled={dialog.loading}
              placeholder="https://github.com/owner/repo/tree/main/path/to/skill"
              type="url"
              value={dialog.sourceUrl}
              onChange={(event) => onUpdate({ sourceUrl: event.target.value, preview: null })}
            />
          </label>
          <div className="remoteSourceCandidatePanel">
            <div className="remoteSourceCandidateHeader">
              <span>Suggested Claude Marketplace matches</span>
              <button className="inlineActionButton" disabled={dialog.searching || dialog.binding} type="button" onClick={onSearch}>
                <RefreshCw aria-hidden="true" size={14} />
                {dialog.searching ? 'Searching...' : 'Search again'}
              </button>
            </div>
            {dialog.searching ? (
              <LoadingNotice compact>
                Searching Claude Marketplace in the background. You can paste a GitHub URL or close this dialog while
                results load.
              </LoadingNotice>
            ) : hasCandidates ? (
              <div className="remoteSourceCandidateList">
                {dialog.candidates.map((candidate) => (
                  <div className="remoteSourceCandidateRow" key={candidate.sourceUrl}>
                    <span>
                      <strong>{candidate.repoLabel || candidate.repoUrl}</strong>
                      <small>{candidate.path}</small>
                      {candidate.description ? <small>{candidate.description}</small> : null}
                    </span>
                    <div className="remoteSourceCandidateMeta">
                      <small>score {candidate.score}</small>
                      <div className="remoteSourceCandidateActions">
                        <button
                          className="button secondary"
                          disabled={!candidate.sourceUrl}
                          type="button"
                          onClick={() => onViewCandidate(candidate)}
                        >
                          View
                        </button>
                        <button
                          className="button primary"
                          disabled={dialog.loading || dialog.binding || !candidate.sourceUrl}
                          type="button"
                          onClick={() => onBindCandidate(candidate)}
                        >
                          Bind
                        </button>
                      </div>
                    </div>
                    {candidate.matchReasons.length > 0 ? (
                      <div className="remoteSourceCandidateReasons">
                        {candidate.matchReasons.slice(0, 3).map((reason) => (
                          <small key={reason}>{reason}</small>
                        ))}
                      </div>
                    ) : null}
                  </div>
                ))}
              </div>
            ) : dialog.searched ? (
              <div className="remoteSourceCandidateNotice">
                {dialog.searchError || 'No Claude Marketplace candidates found. Paste a URL manually.'}
              </div>
            ) : null}
          </div>
          {dialog.preview ? (
            <div className="sourceBindingPreview">
              <strong>{dialog.preview.statusLabel}</strong>
              <span>{formatRemoteRefBehavior(dialog.preview)}</span>
              {dialog.preview.message ? <small>{dialog.preview.message}</small> : null}
            </div>
          ) : null}
          {dialog.error ? <div className="formError">{dialog.error}</div> : null}
          <div className="remoteImportFooter">
            <button className="button secondary" disabled={dialog.binding} type="button" onClick={onClose}>
              Cancel
            </button>
            <button className="button primary" disabled={dialog.loading || dialog.binding || !dialog.sourceUrl.trim()} type="submit">
              {dialog.loading ? (
                <>
                  <span className="buttonSpinner" aria-hidden="true" />
                  Verifying...
                </>
              ) : dialog.binding ? (
                <>
                  <span className="buttonSpinner" aria-hidden="true" />
                  Binding...
                </>
              ) : (
                'Verify and Bind Source'
              )}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}

function RemoteSourceCandidateBindDialog({ dialog, skillName, onClose, onConfirm }) {
  const candidate = dialog.candidate || {};
  const canConfirm = dialog.preview && dialog.preview.validation !== 'mismatch' && !dialog.loading && !dialog.binding;

  return (
    <div
      className="modalBackdrop"
      role="presentation"
      onMouseDown={(event) => closeOnBackdropClick(event, onClose)}
    >
      <section
        className="remoteImportDialog remoteSourceConfirmDialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="remote-source-confirm-title"
      >
        <div className="importSheetHeader">
          <div>
            <h2 id="remote-source-confirm-title">Bind source</h2>
            <p>Confirm the GitHub source for {skillName} after validation passes.</p>
          </div>
          <button
            className="iconButton"
            disabled={dialog.binding}
            type="button"
            aria-label="Close source confirmation"
            onClick={onClose}
          >
            <X aria-hidden="true" />
          </button>
        </div>
        <div className="remoteImportForm">
          <div className="remoteSourceCandidateConfirmSummary">
            <strong>{candidate.repoLabel || candidate.repoUrl || 'Selected source'}</strong>
            {candidate.path ? <small>{candidate.path}</small> : null}
            {candidate.sourceUrl ? <small>{candidate.sourceUrl}</small> : null}
          </div>

          {dialog.loading ? (
            <LoadingNotice>Checking source...</LoadingNotice>
          ) : dialog.preview ? (
            <div className="sourceBindingPreview">
              <strong>{dialog.preview.statusLabel}</strong>
              <span>{formatRemoteRefBehavior(dialog.preview)}</span>
              {dialog.preview.message ? <small>{dialog.preview.message}</small> : null}
            </div>
          ) : null}

          {dialog.error ? <div className="formError">{dialog.error}</div> : null}

          <div className="remoteImportFooter">
            <button className="button secondary" disabled={dialog.binding} type="button" onClick={onClose}>
              Cancel
            </button>
            <button className="button primary" disabled={!canConfirm} type="button" onClick={onConfirm}>
              {dialog.binding ? (
                <>
                  <span className="buttonSpinner" aria-hidden="true" />
                  Binding...
                </>
              ) : (
                'Confirm bind'
              )}
            </button>
          </div>
        </div>
      </section>
    </div>
  );
}

function RemoteVersionReviewDialog({ dialog, onActivatePath, onApply, onClose }) {
  const preview = dialog.preview;
  const activeFile =
    preview?.files.find((file) => file.path === dialog.activePath) ||
    preview?.files[0] ||
    null;
  const hasNoFileChanges = Boolean(preview && preview.files.length === 0);
  const allowNoFileChanges =
    hasNoFileChanges && Boolean(preview?.fromVersion && preview?.toVersion && preview.fromVersion !== preview.toVersion);
  const canApply = canApplyRemoteVersionChange({
    allowNoFileChanges,
    files: preview?.files || [],
    loading: dialog.loading || dialog.applying
  });

  return (
    <div
      className="modalBackdrop"
      role="presentation"
      onMouseDown={(event) => closeOnBackdropClick(event, onClose)}
    >
      <section className="syncDialog gitCommitDialog" role="dialog" aria-modal="true" aria-labelledby="remote-version-title">
        <div className="importSheetHeader">
          <div>
            <h2 id="remote-version-title">
              {preview ? `${remoteVersionActionLabel(preview)} ${preview.skillName}` : 'Review version change'}
            </h2>
            <p>{preview ? `${preview.fromVersion} -> ${preview.toVersion}` : 'Loading remote version diff.'}</p>
          </div>
          <button className="iconButton" disabled={dialog.applying} type="button" aria-label="Close version review" onClick={onClose}>
            <X aria-hidden="true" />
          </button>
        </div>
        <div className="gitCommitDialogBody">
          {dialog.loading ? <LoadingNotice>Loading diff...</LoadingNotice> : null}
          {preview ? (
            <div className="gitCommitReview">
              <aside className="gitFilePane">
                <div className="gitFilePaneHeader">
                  <strong>{preview.files.length} files</strong>
                </div>
                <div className="gitFileList">
                  {preview.files.length > 0 ? (
                    preview.files.map((file) => (
                      <button
                        className={activeFile?.path === file.path ? 'gitFileRow remoteFileRow active' : 'gitFileRow remoteFileRow'}
                        key={file.path}
                        type="button"
                        onClick={() => onActivatePath(file.path)}
                      >
                        <span>
                          <strong>{file.path}</strong>
                          <small>{file.label}</small>
                        </span>
                      </button>
                    ))
                  ) : (
                    <div className="gitEmptyState">No file changes.</div>
                  )}
                </div>
              </aside>
              <section className="gitDiffPane" aria-label="Remote version diff">
                <div className="gitDiffHeader">
                  <strong>{activeFile?.path || 'Diff'}</strong>
                  {activeFile ? <span>{activeFile.label}</span> : null}
                </div>
                {hasNoFileChanges ? (
                  <div className="gitDiffEmpty noFileChanges">
                    <strong>No file changes in this skill</strong>
                    <span>Applying records the latest source revision without changing local files.</span>
                  </div>
                ) : activeFile?.binary || activeFile?.tooLarge ? (
                  <div className="gitDiffEmpty">
                    <span>{`${activeFile.oldHash || 'new'} -> ${activeFile.newHash || 'deleted'}`}</span>
                  </div>
                ) : (
                  <GitDiffView diff={activeFile?.diff || ''} />
                )}
              </section>
            </div>
          ) : null}
          {dialog.error ? <div className="formError remoteDialogError">{dialog.error}</div> : null}
        </div>
        <div className="remoteImportFooter remoteDialogFooter">
          <button className="button secondary" disabled={dialog.applying} type="button" onClick={onClose}>
            Cancel
          </button>
          <button className="button primary" disabled={!canApply} type="button" onClick={onApply}>
            {dialog.applying ? (
              <>
                <span className="buttonSpinner" aria-hidden="true" />
                Applying...
              </>
            ) : (
              'Apply change'
            )}
          </button>
        </div>
      </section>
    </div>
  );
}

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
    <div
      className="modalBackdrop"
      role="presentation"
      onMouseDown={(event) => closeOnBackdropClick(event, onClose)}
    >
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

function ImportReview({
  candidates,
  errors = [],
  onClose,
  onImport,
  onToggleAll,
  onToggleSelected,
  onTypeChange,
  status,
  subtitle = 'Confirm each skill type before SkillBox copies it into the managed store.',
  title = 'Import Review'
}) {
  const [isImportedExpanded, setIsImportedExpanded] = useState(true);
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
    <div
      className="modalBackdrop"
      role="presentation"
      onMouseDown={(event) => closeOnBackdropClick(event, onClose)}
    >
      <section className="importSheet" role="dialog" aria-modal="true" aria-labelledby="import-review-title">
        <div className="importSheetHeader">
          <div>
            <h2 id="import-review-title">{title}</h2>
            <p>{subtitle}</p>
          </div>
          <button className="iconButton" type="button" aria-label="Close import review" onClick={onClose}>
            x
          </button>
        </div>

        <div className="candidateList">
          {errors.length > 0 ? (
            <div className="workspaceSkillError">
              {errors.length} scan {errors.length === 1 ? 'issue' : 'issues'} found.
            </div>
          ) : null}
          {candidates.length === 0 && errors.length === 0 ? (
            <div className="emptyState dashboardEmptyState workspaceSkillEmptyState">
              <strong>No skills found</strong>
              <span>This workspace has no importable SKILL.md directories yet.</span>
            </div>
          ) : null}
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
        <span className="candidateUsage">Calls {candidate.usageCount || 0}</span>
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
                      <strong>Active workspaces</strong>
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
                <AgentIconStack
                  agents={skill.installedAgents}
                  emptyLabel="No deployed workspace"
                  labelPrefix="Deploy workspaces"
                />
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
      <span className="footerIcon">
        <Icon name={icon} />
      </span>
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
  if (name === 'gauge') {
    return <Gauge aria-hidden="true" />;
  }

  if (name === 'folder-code') {
    return <FolderCode aria-hidden="true" />;
  }

  if (name === 'history') {
    return <HistoryIcon aria-hidden="true" />;
  }

  if (name === 'settings-2' || name === 'settings') {
    return <Settings2 aria-hidden="true" />;
  }

  if (name === 'message-circle-question-mark' || name === 'help') {
    return <MessageCircleQuestionMark aria-hidden="true" />;
  }

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

  if (name === 'workspaces') {
    return (
      <svg aria-hidden="true" viewBox="0 0 24 24">
        <path d="M4 5h6v6H4z" />
        <path d="M14 5h6v6h-6z" />
        <path d="M4 15h6v4H4z" />
        <path d="M14 15h6v4h-6z" />
        <path d="M10 8h4" />
        <path d="M10 17h4" />
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
  const usageCountValue = Number(skill.usageCount ?? skill.usage_count);

  return {
    ...skill,
    sourceRoot,
    contentHash: skill.contentHash || skill.content_hash,
    skillMdPath: skill.skillMdPath || skill.skill_md_path,
    isSymlink,
    type,
    usageCount: Number.isFinite(usageCountValue) && usageCountValue > 0 ? usageCountValue : 0,
    lastUsedAt: skill.lastUsedAt || skill.last_used_at || '',
    status: skill.status || defaultSkillStatus(type)
  };
}

function normalizeRemoteSkillVersions(result = {}) {
  const versions = (result.versions || []).map((version) => ({
    version: version.version || '',
    isCurrent: Boolean(version.isCurrent ?? version.is_current),
    kind: version.kind || '',
    shortLabel: version.shortLabel || version.short_label || version.version || '',
    updatedAt: version.updatedAt || version.updated_at || '',
    message: version.message || '',
    path: version.path || ''
  }));

  return {
    skillName: result.skillName || result.skill_name || '',
    currentVersion: result.currentVersion || result.current_version || '',
    versions
  };
}

function normalizeOperationRecords(result = {}) {
  return (result.operations || []).map((operation) => ({
    id: operation.id || '',
    operationType: operation.type || operation.operationType || operation.operation_type || '',
    status: operation.status || '',
    summary: operation.summary || '',
    error: operation.error || '',
    startedAt: operation.startedAt || operation.started_at || '',
    finishedAt: operation.finishedAt || operation.finished_at || ''
  }));
}

function normalizeHistory(result = {}) {
  const entries = (result?.entries || []).map((entry) => ({
    id: entry.id || '',
    kind: entry.kind || '',
    timestamp: entry.timestamp || '',
    title: entry.title || '',
    subtitle: entry.subtitle || '',
    status: entry.status || '',
    skillName: entry.skillName || entry.skill_name || '',
    agentId: entry.agentId || entry.agent_id || '',
    runtimeRoot: entry.runtimeRoot || entry.runtime_root || '',
    promptExcerpt: entry.promptExcerpt || entry.prompt_excerpt || '',
    operationType: entry.operationType || entry.operation_type || '',
    actor: entry.actor || '',
    entityType: entry.entityType || entry.entity_type || '',
    entityName: entry.entityName || entry.entity_name || '',
    error: entry.error || ''
  }));

  return {
    entries,
    skillUsageCount: numberOrZero(result?.skillUsageCount ?? result?.skill_usage_count),
    operationCount: numberOrZero(result?.operationCount ?? result?.operation_count)
  };
}

function groupHistoryEntriesByDay(entries = []) {
  const groups = [];
  const groupByKey = new Map();

  entries.forEach((entry) => {
    const key = historyDayKey(entry.timestamp);
    const label = historyDayLabel(entry.timestamp);
    if (!groupByKey.has(key)) {
      const group = { key, label, entries: [] };
      groupByKey.set(key, group);
      groups.push(group);
    }
    groupByKey.get(key).entries.push(entry);
  });

  return groups;
}

function historyDayKey(timestamp = '') {
  const date = historyDate(timestamp);
  if (!date) return 'unknown';

  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

function historyDayLabel(timestamp = '') {
  const date = historyDate(timestamp);
  if (!date) return 'Unknown date';

  const today = historyDayKey(new Date().toISOString());
  const yesterdayDate = new Date();
  yesterdayDate.setDate(yesterdayDate.getDate() - 1);
  const yesterday = historyDayKey(yesterdayDate.toISOString());
  const key = historyDayKey(timestamp);

  if (key === today) return 'Today';
  if (key === yesterday) return 'Yesterday';

  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${date.getFullYear()}-${month}-${day}`;
}

function historyDate(timestamp = '') {
  const value = String(timestamp || '').trim();
  if (!value) return null;

  const milliseconds = /^\d+$/.test(value) ? Number(value) * 1000 : Date.parse(value);
  const date = new Date(milliseconds);
  return Number.isFinite(milliseconds) && !Number.isNaN(date.getTime()) ? date : null;
}

function operationStatusTone(status = '') {
  if (status === 'succeeded') return 'green';
  if (status === 'failed') return 'red';
  if (status === 'cancelled') return 'amber';
  return 'slate';
}

function numberOrZero(value) {
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? number : 0;
}

function normalizePreferences(preferences) {
  return {
    skipLocalImportConfirmation: Boolean(
      preferences?.skipLocalImportConfirmation ?? preferences?.skip_local_import_confirmation
    ),
    statusRefreshIntervalMinutes: normalizeStatusRefreshIntervalMinutes(
      preferences?.statusRefreshIntervalMinutes ?? preferences?.status_refresh_interval_minutes
    ),
    remoteUpdateTimeoutSeconds: normalizeRemoteUpdateTimeoutSeconds(
      preferences?.remoteUpdateTimeoutSeconds ?? preferences?.remote_update_timeout_seconds
    )
  };
}

function normalizeUsageHookStatuses(rows) {
  const incoming = Array.isArray(rows) ? rows : [];
  const byTarget = new Map();

  for (const row of incoming) {
    const target = row.target || '';
    if (!target) {
      continue;
    }
    byTarget.set(target, {
      target,
      label: row.label || usageHookTargetLabel(target),
      configPath: row.configPath || row.config_path || '',
      command: row.command || '',
      installed: Boolean(row.installed),
      trustRequired: Boolean(row.trustRequired ?? row.trust_required),
      activationNote: row.activationNote || row.activation_note || '',
      sharedConfigKey: row.sharedConfigKey || row.shared_config_key || target
    });
  }

  return previewUsageHooks.map((fallback) => ({
    ...fallback,
    ...(byTarget.get(fallback.target) || {})
  }));
}

function groupUsageHooksByConfig(hooks) {
  const groups = [];
  const byKey = new Map();

  for (const hook of hooks) {
    const key =
      hook.configPath && hook.command
        ? `${hook.configPath}:${hook.command}`
        : hook.sharedConfigKey || hook.target;
    const existing = byKey.get(key);
    if (existing) {
      existing.labels.push(hook.label);
      existing.installed = existing.installed || hook.installed;
      existing.trustRequired = existing.trustRequired || hook.trustRequired;
      existing.activationNote = existing.activationNote || hook.activationNote;
      continue;
    }

    const group = {
      key,
      target: hook.target,
      labels: [hook.label],
      label: hook.label,
      configPath: hook.configPath,
      command: hook.command,
      installed: hook.installed,
      trustRequired: hook.trustRequired,
      activationNote: hook.activationNote
    };
    byKey.set(key, group);
    groups.push(group);
  }

  return groups.map((group) => ({
    ...group,
    label: group.labels.join(' / ')
  }));
}

function usageHookBadgeTone(group) {
  if (!group.installed || group.trustRequired) return 'amber';
  return 'green';
}

function usageHookStatusLabel(group) {
  if (!group.installed) return 'Not injected';
  if (group.trustRequired) return 'Needs trust';
  return 'Injected';
}

function usageHookTargetLabel(target) {
  return previewUsageHooks.find((hook) => hook.target === target)?.label || 'Agent';
}

function readPreviewPreferences() {
  try {
    const statusRefreshIntervalMinutes = window.localStorage.getItem(
      previewStatusRefreshIntervalStorageKey
    );
    const remoteUpdateTimeoutSeconds = window.localStorage.getItem(
      previewRemoteUpdateTimeoutStorageKey
    );

    return {
      skipLocalImportConfirmation: window.localStorage.getItem(previewPreferenceStorageKey) === 'true',
      statusRefreshIntervalMinutes: normalizeStatusRefreshIntervalMinutes(
        statusRefreshIntervalMinutes
      ),
      remoteUpdateTimeoutSeconds: normalizeRemoteUpdateTimeoutSeconds(
        remoteUpdateTimeoutSeconds
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

function previewCandidatesForWorkspace(workspace) {
  const agentNeedle = workspace.agentId === 'agents' ? '.agents' : `.${workspace.agentId}`;
  const roots = [
    workspace.path,
    workspace.compactPath,
    workspace.path?.replace('/Users/santos', '~')
  ].filter(Boolean);

  return previewImportCandidates.filter((candidate) => {
    const sourcePath = candidate.sourcePath || '';
    const sourceRoot = candidate.sourceRoot || '';

    if (roots.some((root) => sourcePath.startsWith(root) || sourceRoot.startsWith(root))) {
      return true;
    }

    return agentNeedle && (sourcePath.includes(agentNeedle) || sourceRoot.includes(agentNeedle));
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
