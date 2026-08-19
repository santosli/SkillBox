import { compactPath, defaultSkillStatus, joinPath } from './skills.js';
import { filterHistoryEntries } from './historyEntries.js';

export const previewPaths = {
  root: '~/.skillbox',
  userSkillsRoot: '~/.skillbox/user-skills',
  remoteSkillsRoot: '~/.skillbox/remote-skills',
  databasePath: '~/.skillbox/skillbox.sqlite'
};

export function publicPreviewRequested(search = '') {
  return new URLSearchParams(search).get('public-preview') === '1';
}

export const previewImportCandidates = [
  {
    name: 'release-helper',
    description: 'Prepare release notes and verify project release assets.',
    sourcePath: '~/.agents/skills/release-helper',
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
    name: 'docs-reviewer',
    description: 'Review project documentation for accuracy and broken links.',
    sourcePath: '~/.codex/skills/docs-reviewer',
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

const previewGithubCollectionGroups = [
  {
    id: 'skill-release-notes',
    name: 'release-notes',
    description: 'Draft release notes from reviewed repository changes.',
    usageCount: 8,
    requiresReview: false,
    selectedVariantId: 'variant-release-notes',
    variants: [{
      id: 'variant-release-notes',
      requiresTypeReview: false,
      selectedType: 'remote',
      suggestedTypes: ['remote'],
      candidate: {
        name: 'release-notes',
        description: 'Draft release notes from reviewed repository changes.',
        sourcePath: 'skills/release-notes',
        sourceRoot: 'github://skillbox-labs/skillbox-workflows',
        contentHash: 'collection-release-notes-01',
        suggestedType: 'remote',
        skillType: 'remote',
        importOrigin: 'github-collection',
        isSelected: true,
        conflict: null
      },
      locations: []
    }]
  },
  {
    id: 'skill-design-audit',
    name: 'design-audit',
    description: 'Review interface changes against a product design system.',
    usageCount: 4,
    requiresReview: false,
    selectedVariantId: 'variant-design-audit',
    variants: [{
      id: 'variant-design-audit',
      requiresTypeReview: false,
      selectedType: 'user',
      suggestedTypes: ['user'],
      candidate: {
        name: 'design-audit',
        description: 'Review interface changes against a product design system.',
        sourcePath: 'skills/design-audit',
        sourceRoot: 'github://skillbox-labs/skillbox-workflows',
        contentHash: 'collection-design-audit-01',
        suggestedType: 'user',
        skillType: 'user',
        importOrigin: 'github-collection',
        isSelected: false,
        conflict: null
      },
      locations: []
    }]
  },
  {
    id: 'skill-prompt-linter',
    name: 'prompt-linter',
    description: 'Check prompt files for unsafe or ambiguous instructions.',
    usageCount: 2,
    requiresReview: false,
    selectedVariantId: 'variant-prompt-linter',
    variants: [{
      id: 'variant-prompt-linter',
      requiresTypeReview: false,
      selectedType: null,
      suggestedTypes: ['remote'],
      candidate: {
        name: 'prompt-linter',
        description: 'Check prompt files for unsafe or ambiguous instructions.',
        sourcePath: 'skills/prompt-linter',
        sourceRoot: 'github://skillbox-labs/skillbox-workflows',
        contentHash: 'collection-prompt-linter-01',
        suggestedType: 'remote',
        skillType: 'remote',
        importOrigin: 'github-collection',
        isSelected: false,
        conflict: 'A managed skill with this name has a different reviewed source.'
      },
      locations: []
    }]
  },
  {
    id: 'skill-unsafe-helper',
    name: 'unsafe-helper',
    description: 'Repository child rejected by the untrusted-tree validator.',
    usageCount: 0,
    requiresReview: false,
    selectedVariantId: 'variant-unsafe-helper',
    variants: [{
      id: 'variant-unsafe-helper',
      requiresTypeReview: false,
      selectedType: null,
      suggestedTypes: [],
      candidate: {
        name: 'unsafe-helper',
        description: 'Repository child rejected by the untrusted-tree validator.',
        sourcePath: 'skills/unsafe-helper',
        sourceRoot: 'github://skillbox-labs/skillbox-workflows',
        contentHash: 'collection-unsafe-helper-01',
        suggestedType: null,
        skillType: null,
        importOrigin: 'github-collection',
        importStatus: 'invalid',
        isSelected: false,
        conflict: 'Invalid child: unsafe file entry.'
      },
      locations: []
    }]
  }
];

export const previewImportCandidateGroups = [
  {
    id: 'skill-general-video',
    name: 'general-video',
    description: 'Create product videos with an editable composition workflow.',
    usageCount: 6,
    requiresReview: false,
    selectedVariantId: 'variant-general-video',
    variants: [
      {
        id: 'variant-general-video',
        requiresTypeReview: true,
        selectedType: null,
        suggestedTypes: ['user', 'remote'],
        candidate: {
          ...previewImportCandidates[0],
          name: 'general-video',
          sourcePath: '~/.agents/skills/general-video',
          isSelected: false
        },
        locations: [
          {
            sourcePath: '~/.agents/skills/general-video',
            sourceRoot: '~/.agents/skills',
            realPath: '~/.agents/skills/general-video',
            isSymlink: false,
            suggestedType: 'user',
            suggestionReason: 'inside ~/.agents/skills'
          },
          {
            sourcePath: '~/.claude/skills/general-video',
            sourceRoot: '~/.claude/skills',
            realPath: '~/.claude/skills/general-video',
            isSymlink: false,
            suggestedType: 'user',
            suggestionReason: 'Needs confirm'
          },
          {
            sourcePath: '~/.cursor/skills/general-video',
            sourceRoot: '~/.cursor/skills',
            realPath: '~/.claude/skills/general-video',
            isSymlink: true,
            symlinkTargetPath: '~/.claude/skills/general-video',
            suggestedType: 'user',
            suggestionReason: 'Needs confirm'
          },
          {
            sourcePath: '~/.codex/skills/general-video',
            sourceRoot: '~/.codex/skills',
            realPath: '~/.claude/skills/general-video',
            isSymlink: true,
            symlinkTargetPath: '~/.claude/skills/general-video',
            suggestedType: 'remote',
            suggestionReason: 'inside ~/.codex/skills'
          }
        ]
      }
    ]
  },
  ...previewImportCandidates.slice(1).map((candidate, index) => ({
    id: `skill-preview-${index}`,
    name: candidate.name,
    description: candidate.description,
    usageCount: candidate.usageCount || 0,
    selectedVariantId: candidate.isSelected ? `variant-preview-${index}` : null,
    variants: [{
      id: `variant-preview-${index}`,
      candidate,
      locations: [{
        sourcePath: candidate.sourcePath,
        sourceRoot: candidate.sourceRoot,
        realPath: candidate.sourcePath,
        isSymlink: false
      }]
    }]
  })),
  ...previewGithubCollectionGroups
];

export const previewImportCollections = [
  {
    id: 'collection-skillbox-workflows',
    previewId: 'preview-skillbox-workflows-v1',
    displayName: 'skillbox-workflows',
    sourceKind: 'github_remote',
    sourceUrl: 'https://github.com/skillbox-labs/skillbox-workflows.git',
    requestedReference: 'main',
    canonicalWorktreeRoot: null,
    canonicalRepositoryId: null,
    originUrl: 'https://github.com/skillbox-labs/skillbox-workflows.git',
    branch: 'main',
    detached: false,
    reviewedHeadSha: 'a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0',
    children: [
      {
        id: 'collection-child-general-video',
        groupId: 'skill-general-video',
        variantId: 'variant-general-video',
        name: 'general-video',
        relativePath: 'skills/general-video',
        sourcePath: '~/Projects/skillbox-workflows/skills/general-video',
        realPath: '~/Projects/skillbox-workflows/skills/general-video',
        contentHash: '87b21f5571a7d332',
        snapshotHash: 'snapshot-skillbox-general-video',
        importStatus: 'importable',
        conflict: null,
        usageCount: 6,
        suggestedTypes: ['user', 'remote'],
        requiresTypeReview: true,
        selectedType: null,
        isSelected: false,
        locations: [
          ...previewImportCandidateGroups[0].variants[0].locations
        ],
        unlinkedLocations: []
      },
      {
        id: 'collection-child-release-notes',
        groupId: 'skill-release-notes',
        variantId: 'variant-release-notes',
        name: 'release-notes',
        relativePath: 'skills/release-notes',
        sourcePath: 'skills/release-notes',
        realPath: 'skills/release-notes',
        contentHash: 'collection-release-notes-01',
        snapshotHash: 'snapshot-skillbox-release-notes',
        importStatus: 'importable',
        conflict: null,
        usageCount: 8,
        suggestedTypes: ['remote'],
        requiresTypeReview: false,
        selectedType: 'remote',
        isSelected: true,
        locations: [],
        unlinkedLocations: [],
        diff: 'A skills/release-notes/SKILL.md\n+ A reviewed release workflow\n+ A bounded source note\nM README.md'
      },
      {
        id: 'collection-child-design-audit',
        groupId: 'skill-design-audit',
        variantId: 'variant-design-audit',
        name: 'design-audit',
        relativePath: 'skills/design-audit',
        sourcePath: 'skills/design-audit',
        realPath: 'skills/design-audit',
        contentHash: 'collection-design-audit-01',
        snapshotHash: 'snapshot-skillbox-design-audit',
        importStatus: 'importable',
        conflict: null,
        usageCount: 4,
        suggestedTypes: ['user'],
        requiresTypeReview: false,
        selectedType: 'user',
        isSelected: false,
        locations: [],
        unlinkedLocations: [],
        diff: 'A skills/design-audit/SKILL.md\n+ A design review checklist'
      },
      {
        id: 'collection-child-prompt-linter',
        groupId: 'skill-prompt-linter',
        variantId: 'variant-prompt-linter',
        name: 'prompt-linter',
        relativePath: 'skills/prompt-linter',
        sourcePath: 'skills/prompt-linter',
        realPath: 'skills/prompt-linter',
        contentHash: 'collection-prompt-linter-01',
        snapshotHash: 'snapshot-skillbox-prompt-linter',
        importStatus: 'importable',
        conflict: 'A managed skill with this name has a different reviewed source.',
        usageCount: 2,
        suggestedTypes: ['remote'],
        requiresTypeReview: false,
        selectedType: null,
        isSelected: false,
        locations: [],
        unlinkedLocations: [],
        diff: 'M skills/prompt-linter/SKILL.md\n+ Managed version differs from reviewed source\n- Existing local rule'
      },
      {
        id: 'collection-child-unsafe-helper',
        groupId: 'skill-unsafe-helper',
        variantId: 'variant-unsafe-helper',
        name: 'unsafe-helper',
        relativePath: 'skills/unsafe-helper',
        sourcePath: 'skills/unsafe-helper',
        realPath: 'skills/unsafe-helper',
        contentHash: 'collection-unsafe-helper-01',
        snapshotHash: null,
        importStatus: 'invalid',
        conflict: 'Invalid child: unsafe file entry.',
        usageCount: 0,
        suggestedTypes: [],
        requiresTypeReview: false,
        selectedType: null,
        isSelected: false,
        locations: [],
        unlinkedLocations: [],
        diff: null
      }
    ],
    errors: []
  }
];

export const previewWorkspaces = [
  {
    canonical_path: '/Users/example/.codex/skills',
    path: '/Users/example/.codex/skills',
    kind: 'global',
    source: 'auto',
    agent_id: 'codex',
    profile_id: 'codex',
    profile_name: 'Codex',
    root_key: 'skills',
    format: 'skill_md',
    display_name: 'Codex',
    skill_count: 6,
    imported_skill_count: 5,
    usage_count: 11,
    reference_count: 3,
    last_scan_error_count: 0,
    last_scanned_at: '2026-05-26 08:00:00'
  },
  {
    canonical_path:
      '/Users/example/Projects/demo-app/.agents/skills',
    path:
      '/Users/example/Projects/demo-app/.agents/skills',
    kind: 'user',
    source: 'manual',
    agent_id: 'agents',
    profile_id: 'agents',
    profile_name: 'Agents',
    root_key: 'skills',
    format: 'skill_md',
    display_name: 'demo-app',
    skill_count: 4,
    imported_skill_count: 3,
    usage_count: 7,
    reference_count: 2,
    last_scan_error_count: 0,
    last_scanned_at: '2026-05-26 08:00:00'
  },
  {
    canonical_path: '/Users/example/Projects/design-system/.claude/skills',
    path: '/Users/example/Projects/design-system/.claude/skills',
    kind: 'user',
    source: 'manual',
    agent_id: 'claude-code',
    profile_id: 'claude-code',
    profile_name: 'Claude Code',
    root_key: 'skills',
    format: 'skill_md',
    display_name: 'design-system',
    skill_count: 3,
    imported_skill_count: 3,
    usage_count: 5,
    reference_count: 1,
    last_scan_error_count: 0,
    last_scanned_at: '2026-07-24 08:00:00'
  },
  {
    canonical_path: '/Users/example/Projects/research-notes/.cursor/skills',
    path: '/Users/example/Projects/research-notes/.cursor/skills',
    kind: 'user',
    source: 'manual',
    agent_id: 'cursor',
    profile_id: 'cursor',
    profile_name: 'Cursor',
    root_key: 'skills',
    format: 'skill_md',
    display_name: 'research-notes',
    skill_count: 2,
    imported_skill_count: 1,
    usage_count: 4,
    reference_count: 4,
    last_scan_error_count: 0,
    last_scanned_at: '2026-07-24 08:00:00'
  }
];

export const previewSkills = [
  {
    name: 'release-helper',
    type: 'user',
    description: 'Prepare release notes and verify project release assets.',
    source_root: '~/.agents/skills',
    path: '~/.skillbox/user-skills/release-helper',
    skill_md_path: '~/.skillbox/user-skills/release-helper/SKILL.md',
    status: 'synced',
    usage_count: 8,
    confirmed_count: 3,
    inferred_count: 5,
    reference_count: 2,
    last_used_at: '2026-07-24T09:12:00Z',
    deployments: [{ target_root: '/Users/example/Projects/demo-app/.agents/skills' }]
  },
  {
    name: 'docs-reviewer',
    type: 'remote',
    description: 'Review project documentation for accuracy and broken links.',
    source_root: '~/.codex/skills',
    path: '~/.skillbox/remote-skills/docs-reviewer/current',
    skill_md_path: '~/.skillbox/remote-skills/docs-reviewer/current/SKILL.md',
    status: 'up to date',
    usage_count: 6,
    confirmed_count: 2,
    inferred_count: 4,
    reference_count: 3,
    last_used_at: '2026-07-23T18:40:00Z',
    deployments: [{ target_root: '/Users/example/.codex/skills' }]
  },
  {
    name: 'design-audit',
    type: 'remote',
    description: 'Check interface hierarchy, accessibility, and responsive layout.',
    source_root: '~/.claude/skills',
    path: '~/.skillbox/remote-skills/design-audit/current',
    skill_md_path: '~/.skillbox/remote-skills/design-audit/current/SKILL.md',
    status: 'update available',
    usage_count: 5,
    confirmed_count: 3,
    inferred_count: 2,
    reference_count: 1,
    last_used_at: '2026-07-22T14:20:00Z',
    deployments: [{ target_root: '/Users/example/Projects/design-system/.claude/skills' }]
  },
  {
    name: 'research-digest',
    type: 'remote',
    description: 'Summarize recent research into a concise source-backed digest.',
    source_root: '~/.cursor/skills',
    path: '~/.skillbox/remote-skills/research-digest/current',
    skill_md_path: '~/.skillbox/remote-skills/research-digest/current/SKILL.md',
    status: 'up to date',
    usage_count: 4,
    confirmed_count: 1,
    inferred_count: 3,
    reference_count: 4,
    last_used_at: '2026-07-21T08:00:00Z',
    deployments: [{ target_root: '/Users/example/Projects/research-notes/.cursor/skills' }]
  },
  {
    name: 'test-writer',
    type: 'user',
    description: 'Add focused regression tests for changed product behavior.',
    source_root: '~/.agents/skills',
    path: '~/.skillbox/user-skills/test-writer',
    skill_md_path: '~/.skillbox/user-skills/test-writer/SKILL.md',
    status: 'synced',
    usage_count: 3,
    confirmed_count: 2,
    inferred_count: 1,
    reference_count: 0,
    last_used_at: '2026-07-20T16:15:00Z',
    deployments: [{ target_root: '/Users/example/Projects/demo-app/.agents/skills' }]
  },
  {
    name: 'local-notes-sync',
    type: 'user',
    description: 'Keep project notes organized with a local-first sync workflow.',
    source_root: '~/.codex/skills',
    path: '~/.skillbox/user-skills/local-notes-sync',
    skill_md_path: '~/.skillbox/user-skills/local-notes-sync/SKILL.md',
    status: 'needs sync',
    usage_count: 0,
    confirmed_count: 0,
    inferred_count: 0,
    reference_count: 3,
    last_used_at: '',
    deployments: []
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

export function previewUserSkillsInboundStatus(mode = 'behind') {
  const isDiverged = mode === 'diverged';
  const isDirty = mode === 'dirty';
  const isRemoteOnly = mode === 'remote-only';
  return {
    repo_path: previewPaths.userSkillsRoot,
    branch: 'main',
    remote_url: 'git@example.com:santosli/user-skills.git',
    worktree_state: isDirty || isRemoteOnly ? 'dirty' : 'clean',
    relation: isDiverged ? 'diverged' : isRemoteOnly ? 'remote_only' : 'behind',
    local_sha: isRemoteOnly ? null : '27f71e4',
    remote_sha: '4b6a204',
    merge_base_sha: '27f71e4',
    ahead_count: isDiverged ? 1 : 0,
    behind_count: 2,
    fetched_at: '2026-07-30 16:20:00',
    fetch_error: null,
    message: isDiverged
      ? 'Local and remote histories have diverged.'
      : isRemoteOnly
        ? 'Remote main can initialize this empty local repository.'
        : '2 incoming commits are ready to review.'
  };
}

export function previewUserSkillsInbound(mode = 'behind') {
  const status = previewUserSkillsInboundStatus(mode);
  const isDiverged = mode === 'diverged';
  const isDirty = mode === 'dirty';
  return {
    preview_id: 'public-preview-inbound-4b6a204',
    status,
    can_apply: !isDiverged && !isDirty,
    blocked_reason: isDiverged
      ? 'Diverged history must be resolved with normal Git tooling.'
      : isDirty
        ? 'Commit or discard local changes before applying incoming changes.'
        : null,
    repository_files: ['README.md'],
    safety_issues: [],
    conflict_analysis: isDiverged
      ? {
          local_only_commits: 1,
          remote_only_commits: 2,
          both_changed_files: ['release-helper/SKILL.md'],
          both_changed_skills: ['release-helper'],
          likely_conflict_files: ['release-helper/SKILL.md']
        }
      : null,
    skill_changes: [
      {
        skill_name: 'release-helper',
        previous_name: null,
        kind: 'updated',
        files: ['release-helper/SKILL.md'],
        affected_deployments: [
          {
            target_root: '/Users/example/.codex/skills/release-helper',
            profile_id: 'codex',
            profile_name: 'Codex'
          }
        ]
      },
      {
        skill_name: 'incident-review',
        previous_name: null,
        kind: 'added',
        files: ['incident-review/SKILL.md'],
        affected_deployments: []
      }
    ],
    files: [
      {
        path: 'release-helper/SKILL.md',
        status: 'M',
        label: 'Modified',
        diff:
          'diff --git a/release-helper/SKILL.md b/release-helper/SKILL.md\n' +
          '--- a/release-helper/SKILL.md\n' +
          '+++ b/release-helper/SKILL.md\n' +
          '@@\n' +
          '-description: Prepare release notes.\n' +
          '+description: Prepare release notes and verify signed assets.\n'
      },
      {
        path: 'incident-review/SKILL.md',
        status: 'A',
        label: 'Added',
        diff:
          'diff --git a/incident-review/SKILL.md b/incident-review/SKILL.md\n' +
          'new file mode 100644\n' +
          '--- /dev/null\n' +
          '+++ b/incident-review/SKILL.md\n' +
          '@@\n' +
          '+name: incident-review\n'
      },
      {
        path: 'README.md',
        status: 'M',
        label: 'Modified',
        diff:
          'diff --git a/README.md b/README.md\n' +
          '--- a/README.md\n' +
          '+++ b/README.md\n' +
          '@@\n' +
          '+Repository notes for shared user skills.\n'
      }
    ]
  };
}

export function previewHistory(filter = 'all') {
  const now = Date.now();
  const entries = [
    {
      id: 'preview-usage-release-helper',
      kind: 'skill_usage',
      timestamp: new Date(now - 12 * 60 * 1000).toISOString(),
      title: 'Skill call: release-helper',
      subtitle: 'codex in ~/.agents/skills',
      skill_name: 'release-helper',
      agent_id: 'codex',
      runtime_root: '~/.agents/skills'
    },
    {
      id: 'preview-operation-install',
      kind: 'operation',
      timestamp: Math.floor((now - 42 * 60 * 1000) / 1000).toString(),
      title: 'Installed docs-reviewer',
      subtitle: 'install_remote_skill by desktop',
      status: 'succeeded',
      operation_type: 'install_remote_skill',
      actor: 'desktop',
      entity_type: 'skill',
      entity_name: 'docs-reviewer'
    },
    {
      id: 'preview-reference-research-digest',
      kind: 'usage_reference',
      timestamp: new Date(now - 70 * 60 * 1000).toISOString(),
      title: 'History reference: research-digest',
      subtitle: 'cursor in ~/.cursor/skills',
      skill_name: 'research-digest',
      agent_id: 'cursor',
      runtime_root: '~/.cursor/skills'
    },
    {
      id: 'preview-usage-design-audit',
      kind: 'skill_usage',
      timestamp: new Date(now - 2 * 60 * 60 * 1000).toISOString(),
      title: 'Skill call: design-audit',
      subtitle: 'claude-code in ~/.claude/skills',
      skill_name: 'design-audit',
      agent_id: 'claude-code',
      runtime_root: '~/.claude/skills'
    }
  ];

  return {
    skill_usage_count: 3,
    skill_reference_count: 1,
    operation_count: 2,
    entries: filterHistoryEntries(entries, filter)
  };
}

export function previewUsageRankings(filters = {}) {
  const rows = [
    {
      rank: 1,
      skill_name: 'release-helper',
      kind: 'user',
      managed: true,
      usage_count: 4,
      confirmed_count: 1,
      inferred_count: 3,
      reference_count: 2,
      last_used_at: '2026-07-10T21:12:00Z'
    },
    {
      rank: 2,
      skill_name: 'docs-reviewer',
      kind: 'user',
      managed: true,
      usage_count: 3,
      confirmed_count: 2,
      inferred_count: 1,
      reference_count: 0,
      last_used_at: '2026-07-22T19:02:00Z'
    },
    {
      rank: 3,
      skill_name: 'design-audit',
      kind: null,
      managed: false,
      usage_count: 2,
      confirmed_count: 0,
      inferred_count: 2,
      reference_count: 1,
      last_used_at: '2026-07-18T11:20:00Z'
    },
    {
      rank: 4,
      skill_name: 'test-writer',
      kind: null,
      managed: false,
      system: true,
      usage_count: 1,
      confirmed_count: 1,
      inferred_count: 0,
      reference_count: 0,
      last_used_at: '2026-07-18T11:18:00Z'
    },
    {
      rank: 5,
      skill_name: 'research-digest',
      kind: 'remote',
      managed: true,
      usage_count: 1,
      confirmed_count: 1,
      inferred_count: 0,
      reference_count: 2,
      last_used_at: '2026-06-27T23:21:00Z'
    },
    {
      rank: 6,
      skill_name: 'local-notes-sync',
      kind: 'user',
      managed: true,
      usage_count: 0,
      confirmed_count: 0,
      inferred_count: 0,
      reference_count: 3,
      last_used_at: ''
    },
    {
      rank: 7,
      skill_name: 'workspace-bootstrap',
      kind: 'user',
      managed: true,
      usage_count: 0,
      confirmed_count: 0,
      inferred_count: 0,
      reference_count: 0,
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
    total_calls: filteredRows.reduce(
      (total, row) => total + row.usage_count,
      0
    ),
    total_observed_calls: filteredRows.reduce(
      (total, row) => total + row.usage_count,
      0
    ),
    total_confirmed_calls: filteredRows.reduce(
      (total, row) => total + row.confirmed_count,
      0
    ),
    total_inferred_calls: filteredRows.reduce(
      (total, row) => total + row.inferred_count,
      0
    ),
    total_history_references: filteredRows.reduce(
      (total, row) => total + row.reference_count,
      0
    ),
    coverage: {
      earliest_event_at: '2026-06-24T09:15:00Z',
      latest_event_at: '2026-07-21T08:00:00Z',
      confirmed_calls: filteredRows.reduce(
        (total, row) => total + row.confirmed_count,
        0
      ),
      inferred_calls: filteredRows.reduce(
        (total, row) => total + row.inferred_count,
        0
      ),
      history_references: filteredRows.reduce(
        (total, row) => total + row.reference_count,
        0
      ),
      source_counts: [
        { source: 'agent_hook', evidence_class: 'confirmed', count: 4 },
        { source: 'codex_session_backfill', evidence_class: 'inferred', count: 7 },
        { source: 'cursor_agent_transcript_read', evidence_class: 'inferred', count: 2 },
        { source: 'cursor_session_backfill', evidence_class: 'reference', count: 8 }
      ],
      agent_hook_calls: 4,
      codex_session_backfill_calls: 18,
      claude_code_session_backfill_calls: 3,
      cursor_session_backfill_calls: 2,
      other_observed_calls: 0,
      scanned_codex_session_files: 12,
      scanned_codex_turns: 84,
      scanned_claude_code_session_files: 8,
      scanned_cursor_sessions: 6,
      scanned_cursor_transcript_files: 12
    },
    rows: filteredRows
  };
}

export function previewCandidatesForWorkspace(workspace) {
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

    return false;
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
