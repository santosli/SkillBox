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
