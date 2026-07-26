import { workspaceAgentIcon } from './agentWorkspaceIcons.js';

export const sidebarItems = [
  { id: 'dashboard', label: 'Dashboard', icon: 'gauge' },
  { id: 'workspaces', label: 'Workspaces', icon: 'folder-code' },
  { id: 'rankings', label: 'Rankings', icon: 'chart-no-axes-column-increasing' },
  { id: 'history', label: 'History', icon: 'history' }
];

// Sidebar and footer icon names resolve to lucide-react components by default.
export const sidebarIconConvention = 'lucide-react';

export const helpIssueUrl = 'https://github.com/santosli/SkillBox/issues';

export const sidebarFooterItems = [
  { id: 'settings', label: 'Settings', icon: 'settings-2' },
  { id: 'help', label: 'Help', icon: 'message-circle-question-mark', url: helpIssueUrl }
];

export const workspaceCardMetaLabels = ['Scope', 'Skills', 'Imported', 'Calls'];

export function normalizeWorkspace(workspace = {}) {
  const canonicalPath = workspace.canonicalPath || workspace.canonical_path || '';
  const path = workspace.path || workspace.displayPath || canonicalPath;
  const kind = String(workspace.kind || 'user').toLowerCase();
  const source = String(workspace.source || 'auto').toLowerCase();
  const agentId = workspace.agentId || workspace.agent_id || '';
  const profileId = workspace.profileId || workspace.profile_id || 'custom-skill-md';
  const profileName = workspace.profileName || workspace.profile_name || 'Custom SKILL.md';
  const rootKey = workspace.rootKey || workspace.root_key || 'exact';
  const format = workspace.format || 'skill_md';
  const compactPath = compactWorkspacePath(path || canonicalPath);
  const displayName = workspace.displayName
    || workspace.display_name
    || workspaceDisplayName(path || canonicalPath, profileName, kind);
  const agentIcon = workspaceAgentIcon({
    canonicalPath,
    path,
    kind,
    agentId,
    profileId,
    profileName,
    rootKey,
    format,
    compactPath,
    displayName
  });
  const skillCount = numberOrZero(workspace.skillCount ?? workspace.skill_count);
  const importedSkillCount = numberOrZero(
    workspace.importedSkillCount ?? workspace.imported_skill_count
  );
  const usageCount = numberOrZero(workspace.usageCount ?? workspace.usage_count);
  const referenceCount = numberOrZero(workspace.referenceCount ?? workspace.reference_count);
  const lastScanErrorCount = numberOrZero(
    workspace.lastScanErrorCount ?? workspace.last_scan_error_count
  );

  return {
    ...workspace,
    canonicalPath,
    path,
    compactPath,
    kind,
    kindLabel: labelize(kind),
    source,
    sourceLabel: labelize(source),
    agentId,
    profileId,
    profileName,
    rootKey,
    format,
    agentIcon,
    agentLabel: profileName,
    displayName,
    skillCount,
    importedSkillCount,
    usageCount,
    referenceCount,
    lastScanErrorCount,
    lastScanError: workspace.lastScanError || workspace.last_scan_error || '',
    lastScannedAt: workspace.lastScannedAt || workspace.last_scanned_at || ''
  };
}

export function normalizeWorkspaces(value) {
  if (!Array.isArray(value)) {
    return [];
  }

  return value.map(normalizeWorkspace);
}

export function normalizeWorkspaceSetupPreview(value = {}) {
  const roots = Array.isArray(value.roots) ? value.roots : [];
  return {
    previewId: String(value.preview_id || value.previewId || ''),
    selectedPath: String(value.selected_path || value.selectedPath || ''),
    kind: value.kind === 'global' ? 'global' : 'user',
    mode: String(value.mode || ''),
    roots: roots.map((root) => ({
      path: String(root.path || ''),
      relativePath: String(root.relative_path || root.relativePath || ''),
      agentId: String(root.agent_id || root.agentId || ''),
      profileId: String(root.profile_id || root.profileId || 'custom-skill-md'),
      profileName: String(root.profile_name || root.profileName || root.label || 'Custom SKILL.md'),
      rootKey: String(root.root_key || root.rootKey || 'exact'),
      format: String(root.format || 'skill_md'),
      label: String(root.label || 'Skills folder'),
      exists: Boolean(root.exists),
      recommended: Boolean(root.recommended)
    }))
  };
}

export function workspaceCounts(workspaces = []) {
  return workspaces.reduce(
    (counts, workspace) => {
      counts.total += 1;
      counts.skills += numberOrZero(workspace.skillCount ?? workspace.skill_count);
      counts.imported += numberOrZero(
        workspace.importedSkillCount ?? workspace.imported_skill_count
      );
      counts.errors += numberOrZero(
        workspace.lastScanErrorCount ?? workspace.last_scan_error_count
      );
      if ((workspace.kind || '').toLowerCase() === 'global') {
        counts.global += 1;
      } else {
        counts.user += 1;
      }
      if ((workspace.source || '').toLowerCase() === 'manual') {
        counts.manual += 1;
      }
      return counts;
    },
    { total: 0, global: 0, user: 0, manual: 0, skills: 0, imported: 0, errors: 0 }
  );
}

export function workspaceTypeTabs(counts = {}) {
  return [
    { id: 'all', label: 'All', count: numberOrZero(counts.total) },
    { id: 'global', label: 'Global', count: numberOrZero(counts.global) },
    { id: 'user', label: 'User', count: numberOrZero(counts.user) }
  ];
}

export function workspaceMatchesTypeFilter(workspace = {}, type = 'all') {
  const normalizedType = String(type || 'all').toLowerCase();
  return normalizedType === 'all' || String(workspace.kind || '').toLowerCase() === normalizedType;
}

export function workspaceMatchesFilters(workspace = {}, { query = '', type = 'all' } = {}) {
  if (!workspaceMatchesTypeFilter(workspace, type)) {
    return false;
  }

  const normalizedQuery = String(query || '').trim().toLowerCase();
  if (!normalizedQuery) {
    return true;
  }

  return [
    workspace.displayName,
    workspace.compactPath,
    workspace.path,
    workspace.canonicalPath,
    workspace.agentLabel,
    workspace.agentId
  ].some((value) => String(value || '').toLowerCase().includes(normalizedQuery));
}

export function workspaceSkillReviewMeta(workspace = {}) {
  const title = `${workspace.displayName || 'Workspace'} skills`;
  const subtitle = workspace.compactPath || workspace.path || 'Not available';
  const noticePrefix = `${workspace.displayName || 'Workspace'}:`;

  return { title, subtitle, noticePrefix };
}

export function workspaceDeployPickerRows(workspaces = [], deployments = []) {
  const deployedRoots = new Set();

  for (const deployment of deployments) {
    for (const value of deploymentRootValues(deployment)) {
      const key = workspacePathKey(value);
      if (key) {
        deployedRoots.add(key);
      }
    }
  }

  return normalizeWorkspaces(workspaces).map((workspace) => {
    const aliases = [workspace.canonicalPath, workspace.path].map(workspacePathKey).filter(Boolean);
    const isDeployed = aliases.some((alias) => deployedRoots.has(alias));

    return {
      ...workspace,
      isDeployed,
      isSelected: isDeployed,
      compatibility: null,
      compatibilityLoading: false,
      compatibilityError: '',
      confirmWarnings: false
    };
  });
}

export function workspaceDeploymentChanges(rows = []) {
  return rows.reduce(
    (changes, row) => {
      if (row.isSelected && !row.isDeployed) {
        changes.deploy.push(row);
      } else if (!row.isSelected && row.isDeployed) {
        changes.undeploy.push(row);
      }
      return changes;
    },
    { deploy: [], undeploy: [] }
  );
}

export function workspaceDeployRequiresConfirmation(changes = {}) {
  return Array.isArray(changes.undeploy) && changes.undeploy.length > 0;
}

export function workspaceDeployChangeCount(changes = {}) {
  return numberOrZero(changes.deploy?.length) + numberOrZero(changes.undeploy?.length);
}

export function workspaceDeployCanSubmit(rows = []) {
  return rows.every((row) => {
    if (!row.isSelected || row.isDeployed) return true;
    if (row.compatibilityLoading || !row.compatibility) return false;
    if (row.compatibility.status === 'blocked') return false;
    return row.compatibility.status !== 'warnings' || row.confirmWarnings;
  });
}

function deploymentRootValues(deployment) {
  if (!deployment) {
    return [];
  }
  if (typeof deployment === 'string') {
    return [deployment];
  }
  return [
    deployment.targetRoot,
    deployment.target_root,
    deployment.targetPath,
    deployment.target_path
  ];
}

function workspacePathKey(value = '') {
  return String(value || '').replace(/\/+$/g, '');
}

function compactWorkspacePath(value = '') {
  return String(value || 'Not available').replace(/^\/Users\/[^/]+(?=\/|$)/, '~');
}

function workspaceDisplayName(path = '', profileName = '', kind = 'user') {
  if (kind === 'global') {
    return profileName || pathSegment(path) || 'Local';
  }

  const segments = String(path || '').split('/').filter(Boolean);
  const rootName = segments.at(-1) || '';
  const parentName = segments.at(-2) || '';

  if (rootName === 'skills' && parentName.startsWith('.')) {
    return segments.at(-3) || profileName || 'Local';
  }

  if (rootName === 'skills') {
    return parentName || profileName || 'Local';
  }

  return rootName || profileName || 'Local';
}

function pathSegment(path = '') {
  return String(path || '').split('/').filter(Boolean).at(-1) || '';
}

function labelize(value = '') {
  const label = String(value || '').replace(/[-_]/g, ' ');
  return label ? label.charAt(0).toUpperCase() + label.slice(1) : '';
}

function numberOrZero(value) {
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? number : 0;
}
