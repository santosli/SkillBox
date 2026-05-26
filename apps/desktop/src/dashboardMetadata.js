import { remoteSkillRowStatus } from './skillStatusRefresh.js';
import { userSkillRowStatus } from './userSkillsGitSync.js';

const tagRules = [
  ['manage', ['manage', 'manager', 'organize', 'maintain']],
  ['doc', ['doc', 'docs', 'document', 'markdown', 'writing']],
  ['code', ['coding', 'development', 'review', 'frontend', 'api']],
  ['obsidian', ['obsidian', 'vault']],
  ['github', ['github', 'git']],
  ['research', ['research', 'search', 'browser']],
  ['sync', ['sync', 'import', 'update', 'deploy']]
];

const agentCatalog = {
  codex: { id: 'codex', label: 'Codex' },
  agents: { id: 'agents', label: 'Agents' },
  claude: { id: 'claude', label: 'Claude' },
  'claude-code': { id: 'claude-code', label: 'Claude Code' },
  cursor: { id: 'cursor', label: 'Cursor' },
  copilot: { id: 'copilot', label: 'Copilot' },
  openclaw: { id: 'openclaw', label: 'OpenClaw' }
};

export function deriveDashboardSkill(skill, userSkillsGit, remoteSkillUpdates, favoriteNames = new Set()) {
  const status = dashboardSkillStatus(skill, userSkillsGit, remoteSkillUpdates);
  const sourceRoot = skill.sourceRoot || skill.source_root || '';

  return {
    ...skill,
    displayTags: deriveTags(skill),
    agentLabel: deriveAgentLabel(sourceRoot),
    installedAgents: deriveInstalledAgents(skill),
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

function deriveAgentLabel(sourceRoot = '') {
  const source = String(sourceRoot);

  if (source.includes('/.codex/skills') || source.includes('~/.codex/skills')) {
    return 'Codex';
  }
  if (source.includes('/.agents/skills') || source.includes('~/.agents/skills')) {
    return 'Agents';
  }
  return 'Local';
}

function compactSourceLabel(sourceRoot = '') {
  const source = String(sourceRoot || 'Local').replace('/Users/santos', '~');

  if (source.includes('~/.codex/skills')) return '~/.codex/skills';
  if (source.includes('~/.agents/skills')) return '~/.agents/skills';
  return source || 'Local';
}

function deriveInstalledAgents(skill) {
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
    pushAgent(agents, agentCatalog.codex);
  }

  return agents;
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
  if (normalized.includes('/.codex/skills') || normalized.includes('~/.codex/skills') || normalized === 'codex') {
    return agentCatalog.codex;
  }
  if (
    normalized.includes('/.agents/skills') ||
    normalized.includes('~/.agents/skills') ||
    normalized === 'agents'
  ) {
    return agentCatalog.agents;
  }
  if (normalized.includes('claude-code') || normalized.includes('claude code') || normalized === 'claude-code') {
    return agentCatalog['claude-code'];
  }
  if (normalized.includes('claude') || normalized === 'anthropic') {
    return agentCatalog.claude;
  }
  if (normalized.includes('cursor')) {
    return agentCatalog.cursor;
  }
  if (normalized.includes('copilot')) {
    return agentCatalog.copilot;
  }
  if (normalized.includes('openclaw')) {
    return agentCatalog.openclaw;
  }

  return null;
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
