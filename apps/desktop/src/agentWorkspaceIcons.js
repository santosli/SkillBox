const agentWorkspaceIconCatalog = {
  codex: {
    id: 'codex',
    label: 'Codex',
    iconClass: 'codex-app',
    iconAsset: 'codex-app',
    aliases: ['codex'],
    pathMarkers: ['/.codex/skills', '~/.codex/skills']
  },
  agents: {
    id: 'agents',
    label: 'Codex CLI',
    iconClass: 'codex-cli',
    iconAsset: 'codex-cli',
    aliases: ['agents', 'codex-cli', 'codex cli'],
    pathMarkers: ['/.agents/skills', '~/.agents/skills']
  },
  claude: {
    id: 'claude',
    label: 'Claude',
    aliases: ['claude', 'anthropic'],
    pathMarkers: ['/.claude/skills', '~/.claude/skills']
  },
  'claude-code': {
    id: 'claude-code',
    label: 'Claude Code',
    aliases: ['claude-code', 'claude code']
  },
  cursor: {
    id: 'cursor',
    label: 'Cursor',
    aliases: ['cursor'],
    pathMarkers: ['/.cursor/skills', '~/.cursor/skills']
  },
  copilot: {
    id: 'copilot',
    label: 'Copilot',
    aliases: ['copilot', 'github copilot'],
    pathMarkers: ['/.copilot/skills', '~/.copilot/skills']
  },
  openclaw: {
    id: 'openclaw',
    label: 'OpenClaw',
    aliases: ['openclaw', 'open claw'],
    pathMarkers: ['/.openclaw/skills', '~/.openclaw/skills']
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

export function agentWorkspaceIconForPath(value = '') {
  const normalized = normalizeLookupValue(value);
  if (!normalized) {
    return null;
  }

  return publicIcon(
    Object.values(agentWorkspaceIconCatalog).find((icon) =>
      icon.pathMarkers?.some((marker) => normalized.includes(marker))
    )
  );
}

export function agentWorkspaceLabel(agentId = '', fallback = '') {
  return agentWorkspaceIconForId(agentId)?.label || fallback;
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
