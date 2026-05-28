import {
  agentWorkspaceIconForId,
  agentWorkspaceIconForPath
} from './agentWorkspaceIcons.js';
import { remoteSkillRowStatus } from './skillStatusRefresh.js';
import { userSkillRowStatus } from './userSkillsGitSync.js';
import { normalizeWorkspaces } from './workspaces.js';

const tagRules = [
  ['manage', ['manage', 'manager', 'organize', 'maintain']],
  ['doc', ['doc', 'docs', 'document', 'markdown', 'writing']],
  ['code', ['coding', 'development', 'review', 'frontend', 'api']],
  ['obsidian', ['obsidian', 'vault']],
  ['github', ['github', 'git']],
  ['research', ['research', 'search', 'browser']],
  ['sync', ['sync', 'import', 'update', 'deploy']]
];

export function deriveDashboardSkill(
  skill,
  userSkillsGit,
  remoteSkillUpdates,
  favoriteNames = new Set(),
  tagOverrides = {},
  workspaces = []
) {
  const status = dashboardSkillStatus(skill, userSkillsGit, remoteSkillUpdates);
  const sourceRoot = skill.sourceRoot || skill.source_root || '';

  return {
    ...skill,
    displayTags: displayTagsForSkill(skill, tagOverrides),
    agentLabel: deriveAgentLabel(sourceRoot),
    installedAgents: deriveInstalledAgents(skill, workspaces),
    sourceLabel: compactSourceLabel(sourceRoot),
    statusLabel: status.label,
    statusTone: status.tone,
    isFavorite: favoriteNames.has(skill.name)
  };
}

export function dashboardFilterOptions(skills = []) {
  const tags = [];
  const agents = [];

  for (const skill of skills) {
    for (const tag of skill.displayTags || []) {
      pushUnique(tags, tag);
    }
    if (skill.agentLabel) {
      pushUnique(agents, skill.agentLabel);
    }
  }

  return { tags, agents };
}

export function normalizeFavoriteNames(value) {
  let parsed = value;

  if (typeof value === 'string') {
    try {
      parsed = JSON.parse(value);
    } catch {
      return [];
    }
  }

  if (!Array.isArray(parsed)) {
    return [];
  }

  return [...new Set(parsed.filter((item) => typeof item === 'string' && item.trim()).map((item) => item.trim()))];
}

export function normalizeDashboardTagOverrides(value) {
  let parsed = value;

  if (typeof value === 'string') {
    try {
      parsed = JSON.parse(value);
    } catch {
      return {};
    }
  }

  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    return {};
  }

  return Object.entries(parsed).reduce((overrides, [skillName, tags]) => {
    const name = String(skillName || '').trim();
    if (!name || !Array.isArray(tags)) {
      return overrides;
    }

    overrides[name] = normalizeEditableTags(tags);
    return overrides;
  }, {});
}

export function normalizeEditableTags(tags = []) {
  if (!Array.isArray(tags)) {
    return [];
  }

  const normalized = tags
    .filter((tag) => typeof tag === 'string')
    .map((tag) =>
      tag
        .trim()
        .toLowerCase()
        .replace(/\s+/g, '-')
        .replace(/[^a-z0-9_-]/g, '')
    )
    .filter(Boolean);

  return [...new Set(normalized)];
}

function dashboardSkillStatus(skill, userSkillsGit, remoteSkillUpdates) {
  if (skill.type === 'user') {
    return userSkillRowStatus(skill, userSkillsGit) || fallbackStatus(skill);
  }

  return remoteSkillRowStatus(skill, remoteSkillUpdates) || fallbackStatus(skill);
}

function fallbackStatus(skill) {
  const normalized = String(skill.status || '').toLowerCase();

  if (normalized.includes('error') || normalized.includes('failed')) {
    return { label: 'Error', tone: 'red' };
  }
  if (skill.type === 'remote') {
    if (normalized.includes('update available') || normalized.includes('new version')) {
      return { label: 'Update available', tone: 'amber' };
    }
    if (normalized.includes('up to date') || normalized.includes('current')) {
      return { label: 'Up to date', tone: 'green' };
    }
    return { label: 'Update not checked', tone: 'slate' };
  }
  if (normalized.includes('needs sync') || normalized.includes('dirty')) {
    return { label: 'Needs sync', tone: 'amber' };
  }
  if (normalized.includes('synced') || normalized.includes('clean')) {
    return { label: 'Synced', tone: 'green' };
  }
  return { label: 'Sync not checked', tone: 'slate' };
}

function deriveTags(skill) {
  const haystack = [
    skill.name,
    skill.description,
    skill.sourceRoot,
    skill.source_root,
    skill.status,
    skill.type
  ]
    .filter(Boolean)
    .join(' ')
    .toLowerCase();
  const tags = tagRules
    .filter(([, needles]) => needles.some((needle) => haystack.includes(needle)))
    .map(([tag]) => tag);

  return tags.length > 0 ? tags : ['general'];
}

function displayTagsForSkill(skill, tagOverrides) {
  const normalizedOverrides = normalizeDashboardTagOverrides(tagOverrides);
  if (Object.prototype.hasOwnProperty.call(normalizedOverrides, skill.name)) {
    return normalizedOverrides[skill.name];
  }

  return deriveTags(skill);
}

function deriveAgentLabel(sourceRoot = '') {
  return agentWorkspaceIconForPath(sourceRoot)?.label || 'Local';
}

function compactSourceLabel(sourceRoot = '') {
  const source = String(sourceRoot || 'Local').replace('/Users/santos', '~');

  if (source.includes('~/.codex/skills')) return '~/.codex/skills';
  if (source.includes('~/.agents/skills')) return '~/.agents/skills';
  return source || 'Local';
}

function deriveInstalledAgents(skill, workspaces = []) {
  const workspaceAgents = deriveInstalledWorkspaceAgents(skill, workspaces);
  if (workspaceAgents.length > 0) {
    return workspaceAgents;
  }

  const agents = [];

  addAgentValues(agents, skill.installedAgents);
  addAgentValues(agents, skill.installed_agents);
  addDeploymentAgents(agents, skill.deployments);
  addDeploymentAgents(agents, skill.deploymentTargets);
  addDeploymentAgents(agents, skill.deployment_targets);

  addAgentFromPath(agents, skill.sourceRoot);
  addAgentFromPath(agents, skill.source_root);
  addAgentFromPath(agents, skill.targetRoot);
  addAgentFromPath(agents, skill.target_root);
  addAgentFromPath(agents, skill.targetPath);
  addAgentFromPath(agents, skill.target_path);

  if (agents.length === 0 && isSkillDeployed(skill)) {
    pushAgent(agents, agentWorkspaceIconForId('codex'));
  }

  return agents;
}

function deriveInstalledWorkspaceAgents(skill, workspaces = []) {
  const normalizedWorkspaces = normalizeWorkspaces(workspaces);
  const workspaceByPath = new Map();
  const agents = [];

  for (const workspace of normalizedWorkspaces) {
    for (const key of [workspace.canonicalPath, workspace.path]) {
      const normalizedKey = pathKey(key);
      if (normalizedKey) {
        workspaceByPath.set(normalizedKey, workspace);
      }
    }
  }

  for (const deployment of deploymentItems(skill)) {
    const workspace = workspaceByPath.get(pathKey(deploymentRoot(deployment)));
    if (!workspace) {
      continue;
    }
    pushAgent(agents, workspaceAgentIcon(workspace));
  }

  return agents;
}

function workspaceAgentIcon(workspace) {
  if (workspace.kind !== 'user') {
    return normalizeAgent(workspace.agentId) || {
      id: `workspace:${workspace.canonicalPath || workspace.path}`,
      label: workspace.displayName || workspace.compactPath || workspace.path || 'Workspace',
      iconClass: 'workspace',
      iconLabel: workspaceInitial(workspace),
      workspace: true
    };
  }

  return {
    id: `workspace:${workspace.canonicalPath || workspace.path}`,
    label: workspace.displayName || workspace.compactPath || workspace.path || 'Workspace',
    iconClass: 'workspace',
    iconLabel: workspaceInitial(workspace),
    workspace: true
  };
}

function workspaceInitial(workspace = {}) {
  return String(workspace.displayName || workspace.path || '?')
    .trim()
    .slice(0, 1)
    .toUpperCase() || '?';
}

function addDeploymentAgents(agents, deployments) {
  if (!Array.isArray(deployments)) {
    return;
  }

  for (const deployment of deployments) {
    if (typeof deployment === 'string') {
      addAgentFromPath(agents, deployment);
      continue;
    }

    addAgentValues(agents, [
      deployment?.agentId,
      deployment?.agent_id,
      deployment?.agent,
      deployment?.targetRoot,
      deployment?.target_root,
      deployment?.targetPath,
      deployment?.target_path
    ]);
  }
}

function deploymentItems(skill) {
  return [
    ...(Array.isArray(skill.deployments) ? skill.deployments : []),
    ...(Array.isArray(skill.deploymentTargets) ? skill.deploymentTargets : []),
    ...(Array.isArray(skill.deployment_targets) ? skill.deployment_targets : [])
  ];
}

function deploymentRoot(deployment) {
  if (typeof deployment === 'string') {
    return deployment;
  }
  return deployment?.targetRoot || deployment?.target_root || '';
}

function pathKey(value = '') {
  return String(value || '').replace(/\/+$/g, '');
}

function addAgentValues(agents, values) {
  const items = Array.isArray(values) ? values : [values];

  for (const value of items) {
    if (!value) {
      continue;
    }
    if (typeof value === 'object') {
      pushAgent(agents, normalizeAgent(value.id || value.agentId || value.agent_id || value.label));
      continue;
    }
    addAgentFromPath(agents, value);
  }
}

function addAgentFromPath(agents, value = '') {
  const agent = normalizeAgent(value);
  if (agent) {
    pushAgent(agents, agent);
  }
}

function normalizeAgent(value = '') {
  const normalized = String(value).toLowerCase();

  if (!normalized) {
    return null;
  }

  return agentWorkspaceIconForPath(value) || agentWorkspaceIconForId(value);
}

function pushAgent(agents, agent) {
  if (agent && !agents.some((item) => item.id === agent.id)) {
    agents.push(agent);
  }
}

function isSkillDeployed(skill) {
  const normalized = String(skill.status || '').toLowerCase();
  return (
    Boolean(skill.isSymlink || skill.is_symlink) ||
    normalized.includes('deployed') ||
    normalized.includes('healthy')
  );
}

function pushUnique(items, item) {
  if (!items.includes(item)) {
    items.push(item);
  }
}
