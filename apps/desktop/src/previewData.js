import { compactPath, defaultSkillStatus, joinPath } from './skills.js';

export const previewPaths = {
  root: '~/.skillbox',
  userSkillsRoot: '~/.skillbox/user-skills',
  remoteSkillsRoot: '~/.skillbox/remote-skills',
  databasePath: '~/.skillbox/skillbox.sqlite'
};

export const previewImportCandidates = [
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

export const previewWorkspaces = [
  {
    canonical_path: '/Users/example/.codex/skills',
    path: '/Users/example/.codex/skills',
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
      '/Users/example/Library/Mobile Documents/iCloud~md~obsidian/Documents/demo-vault/.agents/skills',
    path:
      '/Users/example/Library/Mobile Documents/iCloud~md~obsidian/Documents/demo-vault/.agents/skills',
    kind: 'user',
    source: 'manual',
    agent_id: 'agents',
    display_name: 'demo-vault',
    skill_count: 2,
    imported_skill_count: 1,
    last_scan_error_count: 0,
    last_scanned_at: '2026-05-26 08:00:00'
  }
];

export const previewUsageHooks = [
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

export function previewUserSkillsGitChanges() {
  return {
    repo_path: previewPaths.userSkillsRoot,
    initialized: true,
    branch: 'main',
    remote_url: 'git@example.com:santosli/user-skills.git',
    files: [
      {
        path: 'codex-chat-sync/SKILL.md',
        status: ' M',
        diff:
          'diff --git a/codex-chat-sync/SKILL.md b/codex-chat-sync/SKILL.md\n' +
          '--- a/codex-chat-sync/SKILL.md\n' +
          '+++ b/codex-chat-sync/SKILL.md\n' +
          '@@\n' +
          '+description: Import Codex App history into demo-vault.\n'
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

export function previewHistory() {
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

export function previewUsageRankings(filters = {}) {
  const rows = [
    {
      rank: 1,
      skill_name: 'git-merge-to-main',
      kind: 'user',
      managed: true,
      usage_count: 4,
      last_used_at: '2026-07-10T21:12:00Z'
    },
    {
      rank: 2,
      skill_name: 'black-cat-ai-illustrations',
      kind: 'user',
      managed: true,
      usage_count: 3,
      last_used_at: '2026-07-22T19:02:00Z'
    },
    {
      rank: 3,
      skill_name: 'skill-creator',
      kind: null,
      managed: false,
      usage_count: 2,
      last_used_at: '2026-07-18T11:20:00Z'
    },
    {
      rank: 4,
      skill_name: 'skill-creator',
      kind: null,
      managed: false,
      system: true,
      usage_count: 1,
      last_used_at: '2026-07-18T11:18:00Z'
    },
    {
      rank: 5,
      skill_name: 'last30days',
      kind: 'remote',
      managed: true,
      usage_count: 1,
      last_used_at: '2026-06-27T23:21:00Z'
    },
    {
      rank: 6,
      skill_name: 'codex-chat-sync',
      kind: 'user',
      managed: true,
      usage_count: 0,
      last_used_at: ''
    },
    {
      rank: 7,
      skill_name: 'dida-task-sync',
      kind: 'user',
      managed: true,
      usage_count: 0,
      last_used_at: ''
    }
  ];

  const sourceAwareRows = rows.map((row) => {
    const sourceKind = row.system ? 'system' : 'regular';
    return {
      ...row,
      source_kind: sourceKind,
      source_id: `preview:${sourceKind}:${row.skill_name}`,
      source_runtime_roots: ['/tmp/preview-skills']
    };
  });
  const filteredRows = sourceAwareRows
    .filter((row) => {
      switch (filters.skillType) {
        case 'user':
        case 'remote':
          return row.managed && row.kind === filters.skillType;
        case 'system':
          return row.system;
        default:
          return true;
      }
    })
    .map((row, index) => ({ ...row, rank: index + 1 }));

  return {
    generated_at: '2026-07-22T10:00:00Z',
    range: filters.range || 'last_30_days',
    range_start: '2026-06-22T10:00:00Z',
    range_end: '2026-07-22T10:00:00Z',
    agent_id: filters.agentId || null,
    skill_type: filters.skillType || null,
    workspace_root: filters.workspaceRoot || null,
    total_observed_calls: filteredRows.reduce(
      (total, row) => total + row.usage_count,
      0
    ),
    coverage: {
      earliest_event_at: '2026-06-24T09:15:00Z',
      latest_event_at: '2026-07-21T08:00:00Z',
      agent_hook_calls: 4,
      codex_session_backfill_calls: 18,
      claude_code_session_backfill_calls: 3,
      cursor_session_backfill_calls: 2,
      other_observed_calls: 0,
      scanned_codex_session_files: 12,
      scanned_claude_code_session_files: 8,
      scanned_cursor_sessions: 6
    },
    rows: filteredRows
  };
}

export function previewCandidatesForWorkspace(workspace) {
  const agentNeedle = workspace.agentId === 'agents' ? '.agents' : `.${workspace.agentId}`;
  const roots = [
    workspace.path,
    workspace.compactPath,
    compactPath(workspace.path)
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

export function previewContentHash(value) {
  let hash = 0;
  for (const char of value) {
    hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  }
  return `preview-${hash.toString(16).padStart(8, '0')}`;
}

export function candidateToPreviewSkill(candidate) {
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

export function applyPreviewImportStatuses(candidates, importedSkills) {
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
