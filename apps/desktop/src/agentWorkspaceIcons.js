const agentWorkspaceIconCatalog = {
  codex: {
    id: 'codex',
    label: 'Codex',
    iconClass: 'codex-app',
    iconAsset: 'codex-app',
    aliases: ['codex']
  },
  agents: {
    id: 'agents',
    label: 'Agents',
    iconClass: 'codex-cli',
    iconAsset: 'codex-cli',
    aliases: ['agents']
  },
  claude: {
    id: 'claude',
    label: 'Claude Code',
    iconClass: 'claude-code',
    iconAsset: 'claude-code',
    aliases: ['claude', 'anthropic']
  },
  'claude-code': {
    id: 'claude-code',
    label: 'Claude Code',
    iconClass: 'claude-code',
    iconAsset: 'claude-code',
    aliases: ['claude-code', 'claude code']
  },
  cursor: {
    id: 'cursor',
    label: 'Cursor',
    aliases: ['cursor']
  },
  copilot: {
    id: 'copilot',
    label: 'Copilot',
    aliases: ['copilot', 'github copilot']
  },
  openclaw: {
    id: 'openclaw',
    label: 'OpenClaw',
    aliases: ['openclaw', 'open claw']
  },
  'custom-skill-md': {
    id: 'custom-skill-md',
    label: 'Custom SKILL.md',
    aliases: ['custom-skill-md']
  }
};

export function agentWorkspaceIconForId(value = '') {
  const normalized = normalizeLookupValue(value);
  if (!normalized) {
    return null;
  }

  return publicIcon(
    Object.values(agentWorkspaceIconCatalog).find(
      (icon) => icon.id === normalized || icon.aliases?.includes(normalized)
    )
  );
}

export function agentWorkspaceLabel(agentId = '', fallback = '') {
  return agentWorkspaceIconForId(agentId)?.label || fallback;
}

export function workspaceAgentIcon(workspace = {}) {
  if (String(workspace.kind || '').toLowerCase() === 'user') {
    return workspaceFallbackIcon(workspace);
  }
  return agentWorkspaceIconForId(workspace.profileId || workspace.profile_id)
    || workspaceFallbackIcon(workspace);
}

function workspaceFallbackIcon(workspace = {}) {
  return {
    id: `workspace:${workspace.canonicalPath || workspace.canonical_path || workspace.path}`,
    label: workspace.displayName || workspace.display_name || workspace.compactPath || workspace.path || 'Workspace',
    iconClass: 'workspace',
    iconLabel: workspaceInitial(workspace),
    workspace: true
  };
}

function workspaceInitial(workspace = {}) {
  return String(workspace.displayName || workspace.display_name || workspace.path || '?')
    .trim()
    .slice(0, 1)
    .toUpperCase() || '?';
}

function publicIcon(icon) {
  if (!icon) {
    return null;
  }

  return {
    id: icon.id,
    label: icon.label,
    ...(icon.iconClass ? { iconClass: icon.iconClass } : {}),
    ...(icon.iconAsset ? { iconAsset: icon.iconAsset } : {})
  };
}

function normalizeLookupValue(value = '') {
  return String(value || '').trim().toLowerCase();
}
