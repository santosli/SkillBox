import assert from 'node:assert/strict';
import test from 'node:test';
import {
  filterImportCandidateGroups,
  filterImportCandidateGroupsByQuery,
  filterImportCandidatesByQuery,
  filterWorkspaceSkillCandidates,
  collectionChildTypeState,
  collectionSkillCountLabel,
  importCandidateGroupLocationCount,
  importCandidateGroupTabs,
  normalizeGithubSkillCollectionPreviewResult,
  normalizeImportCollections,
  normalizeImportCandidateGroup,
  normalizeImportCandidateGroups,
  normalizeImportCandidate,
  selectedImportCandidates,
  selectedImportCollectionRequests,
  selectImportCandidateVariant,
  toggleImportCandidateGroupSelection,
  updateImportCandidateGroupType,
  visibleImportCandidates,
  workspaceSkillTabs
} from './importCandidates.js';
import {
  dashboardStatusNotice,
  formatStatusCheckedAt,
  formatStatusNoticeCountdown,
  mergeRemoteSkillUpdates,
  normalizeRemoteSkillUpdates,
  normalizeRemoteUpdateTimeoutSeconds,
  normalizeStatusRefreshIntervalMinutes,
  remoteSkillRowStatus
} from './skillStatusRefresh.js';
import { parseUnifiedDiff } from './gitDiffView.js';
import {
  canApplyRemoteVersionChange,
  formatOperationTimestamp,
  formatRemoteRefBehavior,
  normalizeCompatibilityReport,
  normalizeRemoteInstallPreview,
  normalizeRemoteSourceCandidates,
  normalizeRemoteSourceBindingPreview,
  normalizeRemoteVersionPreview,
  remoteDiffOmissionNotice,
  remoteSkillUpdateVersionLabel,
  remoteVersionActionLabel,
  shouldShowRemoteUpdateSummary
} from './remoteSkills.js';
import {
  canCommitUserSkillsChanges,
  defaultSyncCommitMessage,
  normalizeUserSkillsGitChanges,
  normalizeUserSkillsGitStatus,
  suggestUserSkillsCommitMessage,
  syncNotice,
  waitForNextPaint,
  userSkillsSyncProgressSteps,
  userSkillRowStatus,
  userSyncAction
} from './userSkillsGitSync.js';
import {
  importBatchNotice,
  importRequestItems,
  shouldConfirmLocalImport,
  toggleImportCandidateSelection
} from './importFlow.js';
import {
  browserImportScanOptions,
  createImportScanRequestController,
  createRemoteImportRequestController,
  importScanCommandArgs,
  importScanProgressDetail,
  importScanProgressLabel,
  isImportScanRequestCurrent,
  normalizeImportScanProgress
} from './importScanProgress.js';

test('normalizes staged import scan progress and exposes truthful labels', () => {
  const progress = normalizeImportScanProgress({
    phase: 'grouping Git repositories',
    processed: 12,
    total: 48,
    unique_repositories: 3
  });

  assert.deepEqual(progress, {
    phase: 'grouping Git repositories',
    processed: 12,
    total: 48,
    uniqueRepositories: 3
  });
  assert.equal(importScanProgressLabel(progress), 'Grouping Git repositories');
  assert.equal(importScanProgressDetail(progress), '12 of 48 · 3 repositories');
});

test('import scan request generation ignores stale progress and bounds browser QA delay', () => {
  assert.equal(isImportScanRequestCurrent(4, 4), true);
  assert.equal(isImportScanRequestCurrent(4, 5), false);
  assert.deepEqual(browserImportScanOptions('?import-scan-delay-ms=9999&import-scan-error=1'), {
    delayMs: 2000,
    error: true
  });
  assert.deepEqual(browserImportScanOptions('?import-scan-delay-ms=-10'), {
    delayMs: 0,
    error: false
  });
});

test('builds the camelCase Tauri scan argument required by the production command', () => {
  const args = importScanCommandArgs(7);

  assert.deepEqual(args, { scanId: 7 });
  assert.equal(Object.hasOwn(args, 'scan_id'), false);
});

test('closing and reopening Import Review isolates late scan A from active scan B', () => {
  const controller = createImportScanRequestController();
  const scanA = controller.begin();

  assert.equal(scanA, 1);
  assert.equal(controller.begin(), null, 'duplicate active clicks do not start another scan');

  controller.invalidate();
  const scanB = controller.begin();

  assert.equal(scanB, 3);
  assert.equal(controller.isCurrent(scanA), false);
  assert.equal(controller.isCurrent(scanB), true);

  const applied = [];
  if (controller.isCurrent(scanA)) applied.push('A');
  if (controller.isCurrent(scanB)) applied.push('B');
  assert.deepEqual(applied, ['B']);

  controller.finish(scanB);
  assert.equal(controller.begin(), 4);
});

test('remote preview controller ignores duplicate clicks and late close/reopen results', () => {
  const controller = createRemoteImportRequestController();
  const first = controller.begin();
  assert.equal(first, 1);
  assert.equal(controller.begin(), null);
  controller.invalidate();
  const second = controller.begin();
  assert.equal(second, 3);
  assert.equal(controller.isCurrent(first), false);
  assert.equal(controller.isCurrent(second), true);
  controller.finish(second);
  assert.equal(controller.isCurrent(second), true);
  controller.invalidate();
  assert.equal(controller.isCurrent(second), false);
});

test('normalizes backend is_selected false without selecting importable candidate', () => {
  const candidate = normalizeImportCandidate({
    name: 'system',
    description: 'System skill',
    source_path: '/Users/example/.codex/skills/.system/system',
    source_root: '/Users/example/.codex/skills/.system',
    suggested_type: 'remote',
    import_status: 'importable',
    is_selected: false,
    conflict: null,
    usage_count: 4
  });

  assert.equal(candidate.isSelected, false);
  assert.equal(candidate.usageCount, 4);
});

test('normalizes symlink import candidate source metadata', () => {
  const candidate = normalizeImportCandidate({
    name: 'lark-mail',
    source_path: '/Users/example/.claude/skills/lark-mail',
    source_root: '/Users/example/.claude/skills',
    real_path: '/Users/example/.agents/skills/lark-mail',
    is_symlink: true,
    symlink_target_path: '/Users/example/.agents/skills/lark-mail',
    usage_count: 2
  });

  assert.equal(candidate.isSymlink, true);
  assert.equal(candidate.symlinkTargetPath, '/Users/example/.agents/skills/lark-mail');
  assert.equal(candidate.usageCount, 2);
});

test('normalizes remote install compatibility returned by Rust', () => {
  const preview = normalizeRemoteInstallPreview({
    preview_id: 'install-preview',
    skill_name: 'demo',
    installed_sha: 'abc123',
    files: [{ path: 'SKILL.md', status: 'A', diff: '+name: demo' }],
    compatibility: {
      preview_id: 'compat-preview',
      profile_id: 'claude-code',
      profile_name: 'Claude Code',
      target_root: '/tmp/project/.claude/skills',
      status: 'warnings',
      issues: [
        {
          code: 'unknown_optional_frontmatter',
          severity: 'warning',
          message: 'Unknown optional field: tools',
          suggested_action: 'Review the field before deploying.'
        }
      ]
    }
  });

  assert.deepEqual(preview.compatibility, {
    previewId: 'compat-preview',
    profileId: 'claude-code',
    profileName: 'Claude Code',
    targetRoot: '/tmp/project/.claude/skills',
    status: 'warnings',
    issues: [
      {
        code: 'unknown_optional_frontmatter',
        severity: 'warning',
        message: 'Unknown optional field: tools',
        suggestedAction: 'Review the field before deploying.'
      }
    ]
  });
  assert.equal(normalizeCompatibilityReport(null), null);
});

test('normalizes grouped import candidate source paths', () => {
  const candidate = normalizeImportCandidate({
    name: 'dbs',
    source_path: '/Users/example/.agents/skills/dbs',
    additional_source_paths: [
      '/Users/example/project/.agents/skills/dbs',
      '/Users/example/project/.agents/skills/dbs',
      '/Users/example/.agents/skills/dbs'
    ]
  });

  assert.deepEqual(candidate.additionalSourcePaths, [
    '/Users/example/project/.agents/skills/dbs'
  ]);
});

test('normalizes Rust-owned mixed type suggestions without splitting one content variant', () => {
  const group = normalizeImportCandidateGroup({
    id: 'skill-general-video',
    name: 'general-video',
    description: 'Create product videos.',
    usage_count: 8,
    requires_review: false,
    selected_variant_id: 'variant-shared',
    variants: [
      {
        id: 'variant-shared',
        requires_type_review: true,
        selected_type: null,
        suggested_types: ['user', 'remote'],
        candidate: {
          name: 'general-video',
          source_path: '/Users/example/.agents/skills/general-video',
          suggested_type: 'user',
          import_status: 'importable',
          is_selected: false
        },
        locations: [
          {
            source_path: '/Users/example/.agents/skills/general-video',
            real_path: '/Users/example/.agents/skills/general-video',
            suggested_type: 'user',
            suggestion_reason: 'inside ~/.agents/skills'
          },
          {
            source_path: '/Users/example/.claude/skills/general-video',
            real_path: '/Users/example/.claude/skills/general-video',
            suggested_type: 'user',
            suggestion_reason: 'Needs confirm'
          },
          {
            source_path: '/Users/example/.cursor/skills/general-video',
            real_path: '/Users/example/.claude/skills/general-video',
            is_symlink: true,
            symlink_target_path: '/Users/example/.claude/skills/general-video',
            suggested_type: 'user',
            suggestion_reason: 'Needs confirm'
          },
          {
            source_path: '/Users/example/.codex/skills/general-video',
            real_path: '/Users/example/.claude/skills/general-video',
            is_symlink: true,
            symlink_target_path: '/Users/example/.claude/skills/general-video',
            suggested_type: 'remote',
            suggestion_reason: 'inside ~/.codex/skills'
          }
        ]
      }
    ]
  });

  assert.equal(group.variants.length, 1);
  assert.equal(importCandidateGroupLocationCount(group), 4);
  assert.equal(group.requiresReview, false);
  assert.equal(group.selectedVariantId, 'variant-shared');
  assert.equal(group.variants[0].requiresTypeReview, true);
  assert.equal(group.variants[0].selectedType, null);
  assert.deepEqual(group.variants[0].locations.map((location) => location.sourcePath), [
    '/Users/example/.agents/skills/general-video',
    '/Users/example/.claude/skills/general-video',
    '/Users/example/.cursor/skills/general-video',
    '/Users/example/.codex/skills/general-video'
  ]);
  assert.deepEqual(
    group.variants[0].locations
      .filter((location) => location.isSymlink)
      .map((location) => location.symlinkTargetPath),
    [
      '/Users/example/.claude/skills/general-video',
      '/Users/example/.claude/skills/general-video'
    ]
  );
  assert.equal(group.isSelected, false);
  assert.deepEqual(selectedImportCandidates([group]), []);

  const classified = updateImportCandidateGroupType([group], group.id, 'remote');
  assert.deepEqual(importRequestItems(selectedImportCandidates(classified)), [
    {
      source_path: '/Users/example/.agents/skills/general-video',
      skill_type: 'remote',
      deploy_back_to_source: true
    }
  ]);
});

test('variant review requires an explicit Rust variant choice and submits one primary', () => {
  const [group] = normalizeImportCandidateGroups([
    {
      id: 'skill-demo',
      name: 'demo',
      requires_review: true,
      variants: [
        {
          id: 'variant-a',
          candidate: { name: 'demo', source_path: '/first/demo', import_status: 'importable' },
          locations: [{ source_path: '/first/demo' }, { source_path: '/copy/demo' }]
        },
        {
          id: 'variant-b',
          candidate: { name: 'demo', source_path: '/second/demo', import_status: 'importable' },
          locations: [{ source_path: '/second/demo' }]
        }
      ]
    }
  ]);

  assert.deepEqual(selectedImportCandidates([group]), []);
  const selected = selectImportCandidateVariant([group], group.id, 'variant-b');
  assert.deepEqual(importRequestItems(selectedImportCandidates(selected)), [
    { source_path: '/second/demo', skill_type: 'user', deploy_back_to_source: true }
  ]);
});

test('group search tabs and select-all count one skill while matching every location', () => {
  const groups = normalizeImportCandidateGroups([
    {
      id: 'skill-demo',
      name: 'demo',
      selected_variant_id: 'variant-demo',
      variants: [{
        id: 'variant-demo',
        candidate: { name: 'demo', source_path: '/one/demo', import_status: 'importable', is_selected: false },
        locations: [
          { source_path: '/one/demo' },
          { source_path: '/project/.cursor/skills/demo', is_symlink: true, symlink_target_path: '/one/demo' }
        ]
      }]
    },
    {
      id: 'skill-system',
      name: 'system-skill',
      variants: [{
        id: 'variant-system',
        candidate: { name: 'system-skill', source_path: '/system', import_status: 'system' },
        locations: [{ source_path: '/system' }]
      }]
    }
  ]);

  assert.deepEqual(importCandidateGroupTabs(groups), [
    { id: 'all', label: 'All', count: 2 },
    { id: 'unimported', label: 'Unimported', count: 1 },
    { id: 'imported', label: 'Imported', count: 0 },
    { id: 'system', label: 'System', count: 1 }
  ]);
  assert.deepEqual(filterImportCandidateGroups(groups, 'system').map((group) => group.name), ['system-skill']);
  assert.deepEqual(filterImportCandidateGroupsByQuery(groups, 'cursor skills').map((group) => group.name), ['demo']);
  assert.deepEqual(toggleImportCandidateGroupSelection(groups).map((group) => group.isSelected), [true, false]);
});

test('normalizes Git-backed collection children and submits one selected child request', () => {
  const collections = normalizeImportCollections([{
    id: 'collection-demo',
    preview_id: 'preview-demo',
    display_name: 'skills-repo',
    canonical_worktree_root: '/Users/example/skills-repo',
    reviewed_head_sha: 'abcdef123456',
    children: [{
      id: 'child-demo',
      group_id: 'skill-demo',
      variant_id: 'variant-demo',
      name: 'demo',
      relative_path: 'skills/demo',
      source_path: '/Users/example/skills-repo/skills/demo',
      import_status: 'importable',
      suggested_types: ['user'],
      selected_type: 'user',
      is_selected: true,
      locations: [{ source_path: '/Users/example/skills-repo/skills/demo' }]
    }]
  }]);
  const groups = normalizeImportCandidateGroups([{
    id: 'skill-demo',
    name: 'demo',
    selected_variant_id: 'variant-demo',
    variants: [{
      id: 'variant-demo',
      candidate: { name: 'demo', source_path: '/Users/example/skills-repo/skills/demo', import_status: 'importable', is_selected: true },
      selected_type: 'user',
      locations: [{ source_path: '/Users/example/skills-repo/skills/demo' }]
    }]
  }]);

  const requests = selectedImportCollectionRequests(groups, collections);
  assert.deepEqual(requests, [{
    collectionId: 'collection-demo',
    sourceKind: 'git_worktree',
    sourceUrl: '',
    worktreeRoot: '/Users/example/skills-repo',
    previewId: 'preview-demo',
    selections: [{
      relativePath: 'skills/demo',
      groupId: 'skill-demo',
      variantId: 'variant-demo',
      skillType: 'user'
    }]
  }]);
});

test('normalizes a GitHub collection without inventing a local worktree root', () => {
  const collections = normalizeImportCollections([{
    id: 'github-collection-demo',
    source_kind: 'github_remote',
    source_url: 'https://github.com/acme/skills',
    requested_reference: 'main',
    reviewed_head_sha: '1234567890abcdef',
    children: [{
      id: 'child-demo',
      group_id: 'skill-demo',
      variant_id: 'variant-demo',
      name: 'demo',
      relative_path: 'skills/demo',
      import_status: 'importable',
      selected_type: 'remote',
      is_selected: true,
      locations: []
    }]
  }]);
  const groups = normalizeImportCandidateGroups([{
    id: 'skill-demo',
    name: 'demo',
    selected_variant_id: 'variant-demo',
    variants: [{
      id: 'variant-demo',
      candidate: { name: 'demo', import_status: 'importable', is_selected: true },
      selected_type: 'remote',
      locations: []
    }]
  }]);

  assert.equal(collections[0].sourceKind, 'github_remote');
  assert.equal(collections[0].canonicalWorktreeRoot, '');
  assert.deepEqual(selectedImportCollectionRequests(groups, collections), [{
    collectionId: 'github-collection-demo',
    sourceKind: 'github_remote',
    sourceUrl: 'https://github.com/acme/skills',
    worktreeRoot: '',
    previewId: '',
    selections: [{
      relativePath: 'skills/demo',
      groupId: 'skill-demo',
      variantId: 'variant-demo',
      skillType: 'remote'
    }]
  }]);
});

test('routes structured GitHub collection preview outcomes without parsing human errors', () => {
  const collection = { id: 'collection-demo' };
  assert.deepEqual(
    normalizeGithubSkillCollectionPreviewResult({
      kind: 'collection',
      preview: collection
    }),
    { kind: 'collection', preview: collection }
  );
  assert.deepEqual(
    normalizeGithubSkillCollectionPreviewResult({
      kind: 'single_skill',
      message: 'Use the single-skill preview.'
    }),
    { kind: 'single_skill', message: 'Use the single-skill preview.' }
  );
  assert.deepEqual(
    normalizeGithubSkillCollectionPreviewResult({
      kind: 'explicit_reference_required',
      message: 'Use /tree/<ref>.'
    }),
    { kind: 'explicit_reference_required', message: 'Use /tree/<ref>.' }
  );
  assert.throws(
    () => normalizeGithubSkillCollectionPreviewResult({ error: 'points to one skill' }),
    /invalid result/
  );
});

test('installed-source collections stay on per-skill apply and suppress imported type review', () => {
  const collections = normalizeImportCollections([{
    id: 'installed-source-dbs',
    source_kind: 'installed_source',
    display_name: 'dontbesilent2025/dbskill',
    origin_url: 'https://github.com/dontbesilent2025/dbskill',
    children: [{
      id: 'child-imported',
      group_id: 'skill-imported',
      variant_id: 'variant-imported',
      name: 'git-merge-to-main',
      relative_path: 'skills/git-merge-to-main',
      source_path: '/Users/example/.agents/skills/git-merge-to-main',
      import_status: 'imported',
      requires_type_review: true,
      selected_type: 'user',
      locations: [{ source_path: '/Users/example/.agents/skills/git-merge-to-main' }]
    }]
  }]);

  assert.equal(collections[0].sourceKind, 'installed_source');
  assert.equal(collections[0].canonicalWorktreeRoot, '');
  assert.equal(collections[0].children[0].requiresTypeReview, false);
  assert.equal(collections[0].children[0].selectedType, 'user');
  assert.deepEqual(selectedImportCollectionRequests([], collections), []);

  const importedState = collectionChildTypeState(
    normalizeImportCandidateGroup({
      id: 'skill-imported',
      selected_variant_id: 'variant-imported',
      variants: [{
        id: 'variant-imported',
        candidate: {
          name: 'git-merge-to-main',
          import_status: 'imported',
          is_selected: false
        },
        selected_type: 'user',
        requires_type_review: true
      }]
    }),
    collections[0].children[0]
  );
  assert.equal(importedState.canClassify, false);
  assert.equal(importedState.canSelect, false);
  assert.equal(importedState.needsTypeChoice, false);
  assert.equal(importedState.readOnlyLabel, 'Managed as User');

  const group = normalizeImportCandidateGroup({
    id: 'skill-importable',
    name: 'dbs-action',
    selected_variant_id: 'variant-importable',
    variants: [{
      id: 'variant-importable',
      candidate: {
        name: 'dbs-action',
        source_path: '/Users/example/.agents/skills/dbs-action',
        import_status: 'importable',
        is_selected: true
      },
      selected_type: 'user',
      locations: [{ source_path: '/Users/example/.agents/skills/dbs-action' }]
    }]
  });
  const installedGroup = normalizeImportCollections([{
    id: 'installed-source-dbs',
    source_kind: 'installed_source',
    children: [{ group_id: 'skill-importable', variant_id: 'variant-importable' }]
  }]);
  assert.equal(selectedImportCandidates([group], installedGroup).length, 1);
});

test('collection child type state keeps mixed importable review actionable and blocks system/conflict rows', () => {
  const mixedGroup = normalizeImportCandidateGroup({
    id: 'skill-mixed',
    selected_variant_id: 'variant-mixed',
    variants: [{
      id: 'variant-mixed',
      candidate: {
        name: 'mixed-skill',
        import_status: 'importable',
        is_selected: true
      },
      selected_type: null,
      requires_type_review: true
    }]
  });
  const mixedChild = {
    groupId: 'skill-mixed',
    variantId: 'variant-mixed',
    importStatus: 'importable',
    requiresTypeReview: true,
    selectedType: null,
    conflict: null
  };
  const mixedState = collectionChildTypeState(mixedGroup, mixedChild);
  assert.equal(mixedState.canClassify, true);
  assert.equal(mixedState.canSelect, false);
  assert.equal(mixedState.needsTypeChoice, true);

  const resolvedState = collectionChildTypeState(mixedGroup, {
    ...mixedChild,
    selectedType: 'remote'
  });
  assert.equal(resolvedState.canSelect, true);
  assert.equal(resolvedState.needsTypeChoice, false);

  for (const status of ['system', 'imported']) {
    const state = collectionChildTypeState(mixedGroup, {
      ...mixedChild,
      importStatus: status,
      selectedType: status === 'imported' ? 'remote' : null
    });
    assert.equal(state.canClassify, false);
    assert.equal(state.canSelect, false);
    assert.equal(state.needsTypeChoice, false);
  }

  const conflictState = collectionChildTypeState(mixedGroup, {
    ...mixedChild,
    conflict: 'duplicate managed skill'
  });
  assert.equal(conflictState.canClassify, false);
  assert.equal(conflictState.needsTypeChoice, false);
  assert.equal(conflictState.readOnlyLabel, 'Resolve conflict before import');
});

test('collection skill counts use singular grammar for singleton cards', () => {
  assert.equal(collectionSkillCountLabel(1), '1 skill');
  assert.equal(collectionSkillCountLabel(2), '2 skills');
  assert.equal(collectionSkillCountLabel(0), '0 skills');
});

test('builds workspace skill tabs and separates unimported, imported, and system skills', () => {
  const candidates = [
    normalizeImportCandidate({
      name: 'alpha',
      source_path: '/Users/example/.claude/skills/alpha',
      is_symlink: true,
      import_status: 'importable'
    }),
    normalizeImportCandidate({
      name: 'beta',
      source_path: '/Users/example/.claude/skills/beta',
      is_symlink: true,
      import_status: 'imported'
    }),
    normalizeImportCandidate({
      name: 'gamma',
      source_path: '/Users/example/.claude/skills/gamma',
      import_status: 'importable'
    }),
    normalizeImportCandidate({
      name: 'delta',
      source_path: '/Users/example/.codex/skills/.system/delta',
      is_symlink: true,
      import_status: 'system'
    })
  ];

  assert.deepEqual(workspaceSkillTabs(candidates), [
    { id: 'all', label: 'All', count: 4 },
    { id: 'unimported', label: 'Unimported', count: 2 },
    { id: 'imported', label: 'Imported', count: 1 },
    { id: 'system', label: 'System', count: 1 }
  ]);
  assert.deepEqual(
    filterWorkspaceSkillCandidates(candidates, 'unimported').map((candidate) => candidate.name),
    ['alpha', 'gamma']
  );
  assert.deepEqual(
    filterWorkspaceSkillCandidates(candidates, 'imported').map((candidate) => candidate.name),
    ['beta']
  );
  assert.deepEqual(
    filterWorkspaceSkillCandidates(candidates, 'system').map((candidate) => candidate.name),
    ['delta']
  );
});

test('hides duplicate symlink candidates when the source skill is present', () => {
  const candidates = [
    normalizeImportCandidate({
      name: 'defuddle',
      source_path: '/Users/example/.agents/skills/defuddle',
      real_path: '/Users/example/.agents/skills/defuddle',
      import_status: 'importable'
    }),
    normalizeImportCandidate({
      name: 'defuddle',
      source_path: '/Users/example/.claude/skills/defuddle',
      real_path: '/Users/example/.agents/skills/defuddle',
      is_symlink: true,
      symlink_target_path: '/Users/example/.agents/skills/defuddle',
      import_status: 'importable'
    }),
    normalizeImportCandidate({
      name: 'json-canvas',
      source_path: '/Users/example/.claude/skills/json-canvas',
      real_path: '/Users/example/.agents/skills/json-canvas',
      is_symlink: true,
      symlink_target_path: '/Users/example/.agents/skills/json-canvas',
      import_status: 'importable'
    })
  ];

  assert.deepEqual(
    visibleImportCandidates(candidates).map((candidate) => candidate.sourcePath),
    ['/Users/example/.agents/skills/defuddle', '/Users/example/.claude/skills/json-canvas']
  );
  assert.deepEqual(workspaceSkillTabs(candidates), [
    { id: 'all', label: 'All', count: 2 },
    { id: 'unimported', label: 'Unimported', count: 2 },
    { id: 'imported', label: 'Imported', count: 0 },
    { id: 'system', label: 'System', count: 0 }
  ]);
  assert.deepEqual(
    filterWorkspaceSkillCandidates(candidates, 'unimported').map((candidate) => candidate.sourcePath),
    ['/Users/example/.agents/skills/defuddle', '/Users/example/.claude/skills/json-canvas']
  );
});

test('filters import candidates by name, description, path, and symlink source', () => {
  const candidates = [
    normalizeImportCandidate({
      name: 'defuddle',
      description: 'Extract clean markdown content.',
      source_path: '/Users/example/.agents/skills/defuddle',
      additional_source_paths: ['/Users/example/project/.agents/skills/defuddle'],
      import_status: 'importable'
    }),
    normalizeImportCandidate({
      name: 'json-canvas',
      description: 'Create and edit canvas files.',
      source_path: '/Users/example/.claude/skills/json-canvas',
      real_path: '/Users/example/.agents/skills/json-canvas',
      is_symlink: true,
      symlink_target_path: '/Users/example/.agents/skills/json-canvas',
      import_status: 'importable'
    })
  ];

  assert.deepEqual(
    filterImportCandidatesByQuery(candidates, 'clean markdown').map((candidate) => candidate.name),
    ['defuddle']
  );
  assert.deepEqual(
    filterImportCandidatesByQuery(candidates, 'agents/skills/json').map((candidate) => candidate.name),
    ['json-canvas']
  );
  assert.deepEqual(
    filterImportCandidatesByQuery(candidates, 'project/.agents').map((candidate) => candidate.name),
    ['defuddle']
  );
  assert.deepEqual(filterImportCandidatesByQuery(candidates, 'missing'), []);
});

test('imports only the grouped primary source and leaves duplicate copies unchanged', () => {
  const candidates = [
    normalizeImportCandidate({
      name: 'dbs',
      source_path: '/Users/example/.agents/skills/dbs',
      additional_source_paths: ['/Users/example/project/.agents/skills/dbs'],
      suggested_type: 'user'
    })
  ];

  assert.deepEqual(importRequestItems(candidates), [
    {
      source_path: '/Users/example/.agents/skills/dbs',
      skill_type: 'user',
      deploy_back_to_source: true
    }
  ]);
});

test('always confirms local imports even when a legacy skip preference is stored', () => {
  const candidates = [
    normalizeImportCandidate({
      name: 'dbs',
      source_path: '/Users/example/.agents/skills/dbs',
      import_status: 'importable',
      is_selected: true
    })
  ];

  assert.equal(
    shouldConfirmLocalImport(candidates, { skipLocalImportConfirmation: true }),
    true
  );
});

test('reports grouped import results by skill and source location', () => {
  assert.equal(
    importBatchNotice({
      imported: [
        { name: 'dbs', source_path: '/Users/example/.agents/skills/dbs' },
        { name: 'dbs', source_path: '/Users/example/project/.agents/skills/dbs' }
      ],
      errors: []
    }),
    'Imported 1 skill across 2 locations.'
  );
  assert.equal(
    importBatchNotice({
      imported: [],
      errors: [{ source_path: '/broken', error: 'Existing remote version does not match demo' }]
    }),
    'Imported 0 skills. Failed: /broken: Existing remote version does not match demo.'
  );
});

test('toggles selection only for visible import candidates when duplicates are hidden', () => {
  const visible = [
    normalizeImportCandidate({
      name: 'defuddle',
      source_path: '/Users/example/.agents/skills/defuddle',
      import_status: 'importable',
      is_selected: false
    })
  ];
  const hidden = normalizeImportCandidate({
    name: 'defuddle',
    source_path: '/Users/example/.claude/skills/defuddle',
    is_symlink: true,
    symlink_target_path: '/Users/example/.agents/skills/defuddle',
    import_status: 'importable',
    is_selected: false
  });
  const candidates = [...visible, hidden];

  assert.deepEqual(
    toggleImportCandidateSelection(candidates, visible).map((candidate) => candidate.isSelected),
    [true, false]
  );
});

test('user sync action is setup before remote and hidden for remote skills', () => {
  assert.equal(userSyncAction({ state: 'not_configured' }, 'user'), 'Set up sync');
  assert.equal(userSyncAction({ state: 'clean' }, 'remote'), null);
});

test('user sync action retries failed push and syncs configured remotes', () => {
  assert.equal(userSyncAction({ state: 'push_failed' }, 'user'), 'Retry push');
  assert.equal(userSyncAction({ state: 'dirty' }, 'user'), 'Sync now');
});

test('push failure notice explains manual conflict resolution policy', () => {
  const notice = syncNotice({ state: 'push_failed' });

  assert.match(notice, /Local commit was kept/);
  assert.match(notice, /remote may have diverged/);
  assert.match(notice, /resolve with Git outside SkillBox/);
  assert.match(notice, /retry sync/);
});

test('normalizes user skills git status snake case fields', () => {
  const status = normalizeUserSkillsGitStatus({
    repo_path: '/tmp/.skillbox/user-skills',
    remote_url: 'git@example.com:santosli/user-skills.git',
    last_error: 'push failed',
    state: 'push_failed',
    dirty: true
  });

  assert.deepEqual(status, {
    repoPath: '/tmp/.skillbox/user-skills',
    remoteUrl: 'git@example.com:santosli/user-skills.git',
    branch: '',
    dirty: true,
    rawStatus: '',
    changedPaths: [],
    state: 'push_failed',
    message: 'push failed'
  });
});

test('normalizes changed paths from user skills git status', () => {
  const status = normalizeUserSkillsGitStatus({
    raw_status: '## main\n M codex-chat-sync/SKILL.md\n?? dida-task-sync/SKILL.md\nR  old/SKILL.md -> new/SKILL.md\n'
  });

  assert.deepEqual(status.changedPaths, [
    'codex-chat-sync/SKILL.md',
    'dida-task-sync/SKILL.md',
    'new/SKILL.md'
  ]);
});

test('normalizes user skills git status ignores macOS metadata-only changes', () => {
  const status = normalizeUserSkillsGitStatus({
    dirty: true,
    state: 'dirty',
    raw_status: '## main\n?? .DS_Store\n'
  });

  assert.equal(status.dirty, false);
  assert.equal(status.state, 'clean');
  assert.deepEqual(status.changedPaths, []);
});

test('normalizes user skills git changes and skips macOS metadata by default', () => {
  const changes = normalizeUserSkillsGitChanges({
    repo_path: '/tmp/.skillbox/user-skills',
    files: [
      { path: '.DS_Store', status: '??', diff: 'binary diff' },
      { path: 'alpha/SKILL.md', status: ' M', diff: 'alpha diff' },
      { path: 'beta/SKILL.md', status: '??', diff: 'beta diff' }
    ]
  });

  assert.equal(changes.repoPath, '/tmp/.skillbox/user-skills');
  assert.deepEqual(changes.selectedPaths, ['alpha/SKILL.md', 'beta/SKILL.md']);
  assert.equal(changes.activePath, 'alpha/SKILL.md');
  assert.equal(changes.files[0].label, 'Added');
  assert.equal(changes.files[2].label, 'Added');
  assert.equal(
    suggestUserSkillsCommitMessage(changes.files, changes.selectedPaths),
    'chore(github): sync alpha and beta skills'
  );
});

test('suggests conventional user skills commit messages from selected files', () => {
  const changes = normalizeUserSkillsGitChanges({
    files: [
      { path: 'codex-chat-sync/SKILL.md', status: ' M', diff: 'diff' },
      { path: 'dida-task-sync/SKILL.md', status: '??', diff: 'diff' },
      { path: 'old-skill/SKILL.md', status: ' D', diff: 'diff' }
    ]
  });

  assert.equal(
    suggestUserSkillsCommitMessage(changes.files, ['codex-chat-sync/SKILL.md']),
    'feat(github): update codex-chat-sync skill'
  );
  assert.equal(
    suggestUserSkillsCommitMessage(changes.files, ['dida-task-sync/SKILL.md']),
    'feat(github): add dida-task-sync skill'
  );
  assert.equal(
    suggestUserSkillsCommitMessage(changes.files, ['codex-chat-sync/SKILL.md', 'dida-task-sync/SKILL.md']),
    'chore(github): sync codex-chat-sync and dida-task-sync skills'
  );
});

test('suggests generic user skills commit message for root metadata files', () => {
  const changes = normalizeUserSkillsGitChanges({
    files: [
      { path: '.gitignore', status: '??', diff: 'diff' }
    ]
  });

  assert.equal(
    suggestUserSkillsCommitMessage(changes.files, changes.selectedPaths),
    defaultSyncCommitMessage
  );
});

test('disables user skills commit when no files can be committed', () => {
  assert.equal(canCommitUserSkillsChanges({ files: [], selectedPaths: [] }), false);
  assert.equal(
    canCommitUserSkillsChanges({
      files: [{ path: 'codex-chat-sync/SKILL.md' }],
      selectedPaths: []
    }),
    false
  );
  assert.equal(
    canCommitUserSkillsChanges({
      files: [{ path: 'codex-chat-sync/SKILL.md' }],
      selectedPaths: ['codex-chat-sync/SKILL.md'],
      push: true,
      remoteUrl: ''
    }),
    false
  );
  assert.equal(
    canCommitUserSkillsChanges({
      files: [{ path: 'codex-chat-sync/SKILL.md' }],
      selectedPaths: ['codex-chat-sync/SKILL.md'],
      push: false,
      remoteUrl: ''
    }),
    true
  );
});

test('builds user skills sync progress steps', () => {
  assert.deepEqual(userSkillsSyncProgressSteps({ push: true, selectedCount: 2 }), [
    'Stage 2 files',
    'Create Git commit',
    'Push to origin/main'
  ]);
  assert.deepEqual(userSkillsSyncProgressSteps({ push: false, selectedCount: 1 }), [
    'Stage 1 file',
    'Create Git commit',
    'Skip push'
  ]);
});

test('waits for an animation frame before starting user skills sync work', async () => {
  const originalRequestAnimationFrame = globalThis.requestAnimationFrame;
  const callbacks = [];
  let resolved = false;

  globalThis.requestAnimationFrame = (callback) => {
    callbacks.push(callback);
    return callbacks.length;
  };

  try {
    const promise = waitForNextPaint().then(() => {
      resolved = true;
    });

    assert.equal(resolved, false);
    assert.equal(callbacks.length, 1);

    callbacks.shift()(0);
    await Promise.resolve();

    assert.equal(resolved, false);
    assert.equal(callbacks.length, 1);

    callbacks.shift()(0);
    await promise;

    assert.equal(resolved, true);
  } finally {
    if (originalRequestAnimationFrame) {
      globalThis.requestAnimationFrame = originalRequestAnimationFrame;
    } else {
      delete globalThis.requestAnimationFrame;
    }
  }
});

test('formats last status check timestamps for the dashboard table', () => {
  assert.equal(formatStatusCheckedAt('', new Date('2026-05-26T08:00:00')), 'not checked');
  assert.equal(
    formatStatusCheckedAt(
      String(Math.floor(new Date('2026-05-26T08:00:00').getTime() / 1000)),
      new Date('2026-05-26T08:00:00')
    ),
    '08:00:00'
  );
  assert.equal(
    formatStatusCheckedAt('2026-05-26T00:27:50.818', new Date('2026-05-26T08:00:00')),
    '00:27:50'
  );
  assert.equal(
    formatStatusCheckedAt('2026-05-25T23:05:09.000', new Date('2026-05-26T08:00:00')),
    '2026-05-25 23:05'
  );
});

test('normalizes dashboard auto refresh intervals', () => {
  assert.equal(normalizeStatusRefreshIntervalMinutes(10), 10);
  assert.equal(normalizeStatusRefreshIntervalMinutes('15'), 15);
  assert.equal(normalizeStatusRefreshIntervalMinutes(0), 5);
  assert.equal(normalizeStatusRefreshIntervalMinutes(1441), 5);
});

test('normalizes remote update git timeout seconds', () => {
  assert.equal(normalizeRemoteUpdateTimeoutSeconds(30), 30);
  assert.equal(normalizeRemoteUpdateTimeoutSeconds('45'), 45);
  assert.equal(normalizeRemoteUpdateTimeoutSeconds(4), 30);
  assert.equal(normalizeRemoteUpdateTimeoutSeconds(301), 30);
});

test('formats dashboard status notice countdown labels', () => {
  assert.equal(formatStatusNoticeCountdown(6), 'Closes in 6s');
  assert.equal(formatStatusNoticeCountdown(1), 'Closes in 1s');
  assert.equal(formatStatusNoticeCountdown(0), 'Closing...');
});

test('parses unified diff rows for GitHub-style display', () => {
  const rows = parseUnifiedDiff(
    'diff --git a/example/SKILL.md b/example/SKILL.md\n' +
      'index 1111111..2222222 100644\n' +
      '--- a/example/SKILL.md\n' +
      '+++ b/example/SKILL.md\n' +
      '@@ -2,3 +2,4 @@\n' +
      ' keep\n' +
      '-old line\n' +
      '+new line\n' +
      '+another line'
  );

  assert.deepEqual(
    rows.map((row) => [row.kind, row.oldLine, row.newLine, row.marker, row.content]),
    [
      ['hunk', null, null, '', '@@ -2,3 +2,4 @@'],
      ['context', 2, 2, '', 'keep'],
      ['deletion', 3, null, '-', 'old line'],
      ['addition', null, 3, '+', 'new line'],
      ['addition', null, 4, '+', 'another line']
    ]
  );
});

test('parses simplified hunk headers for preview diffs', () => {
  const rows = parseUnifiedDiff('--- a/file\n+++ b/file\n@@\n-old\n+new\n');

  assert.deepEqual(
    rows.map((row) => [row.kind, row.oldLine, row.newLine, row.marker, row.content]),
    [
      ['hunk', null, null, '', '@@'],
      ['deletion', 1, null, '-', 'old'],
      ['addition', null, 1, '+', 'new']
    ]
  );
});

test('user skill row status follows shared git sync state', () => {
  assert.deepEqual(
    userSkillRowStatus({ type: 'user' }, { state: 'clean' }),
    { label: 'Synced', tone: 'green' }
  );
  assert.equal(userSkillRowStatus({ type: 'remote' }, { state: 'clean' }), null);
});

test('user skill row status marks only changed skills as needing sync', () => {
  const syncStatus = {
    state: 'dirty',
    changedPaths: ['codex-chat-sync/SKILL.md']
  };

  assert.deepEqual(userSkillRowStatus({ name: 'codex-chat-sync', type: 'user' }, syncStatus), {
    label: 'Needs sync',
    tone: 'amber'
  });
  assert.deepEqual(userSkillRowStatus({ name: 'dida-task-sync', type: 'user' }, syncStatus), {
    label: 'Synced',
    tone: 'green'
  });
});

test('remote skill row status follows refreshed update state', () => {
  const updates = normalizeRemoteSkillUpdates({
    checked_at: '1779840000',
    statuses: [
      {
        skill_name: 'grill-me',
        state: 'no_source',
        update_available: false
      },
      {
        skill_name: 'find-skills',
        source_url: 'https://github.com/acme/skills/tree/main/skills/find-skills',
        state: 'update_available',
        update_available: true,
        latest_sha: 'abc123',
        installed_sha: 'def456'
      },
      {
        skill_name: 'frontend-design',
        state: 'up_to_date',
        update_available: false
      },
      {
        skill_name: 'hatch-pet',
        state: 'pinned',
        ref_kind: 'tag',
        tracking: false
      }
    ]
  });

  assert.equal(updates.checkedAt, '1779840000');
  assert.equal(
    updates.statuses.find((status) => status.skillName === 'find-skills').sourceUrl,
    'https://github.com/acme/skills/tree/main/skills/find-skills'
  );
  assert.deepEqual(remoteSkillRowStatus({ name: 'find-skills', type: 'remote' }, updates), {
    label: 'Update available',
    tone: 'amber'
  });
  assert.deepEqual(remoteSkillRowStatus({ name: 'grill-me', type: 'remote' }, updates), {
    label: 'No source',
    tone: 'slate'
  });
  assert.deepEqual(remoteSkillRowStatus({ name: 'frontend-design', type: 'remote' }, updates), {
    label: 'Up to date',
    tone: 'green'
  });
  assert.deepEqual(remoteSkillRowStatus({ name: 'hatch-pet', type: 'remote' }, updates), {
    label: 'Pinned',
    tone: 'blue'
  });
  assert.equal(remoteSkillRowStatus({ name: 'local', type: 'user' }, updates), null);
});

test('single remote update refresh replaces one status without dropping the rest', () => {
  const current = normalizeRemoteSkillUpdates({
    checked_at: '1779840000',
    statuses: [
      { skill_name: 'ui-ux-pro-max', state: 'update_available', update_available: true },
      { skill_name: 'find-skills', state: 'up_to_date', update_available: false }
    ]
  });
  const incoming = normalizeRemoteSkillUpdates({
    checked_at: '1779840300',
    statuses: [
      { skill_name: 'ui-ux-pro-max', state: 'up_to_date', update_available: false }
    ]
  });

  const merged = mergeRemoteSkillUpdates(current, incoming);

  assert.equal(merged.checkedAt, '1779840300');
  assert.deepEqual(
    merged.statuses.map((status) => [status.skillName, status.state]),
    [
      ['ui-ux-pro-max', 'up_to_date'],
      ['find-skills', 'up_to_date']
    ]
  );
});

test('formats remote ref behavior for tracking and pinned sources', () => {
  assert.equal(
    formatRemoteRefBehavior({ refKind: 'branch', reference: 'main', tracking: true }),
    'Tracking branch: main'
  );
  assert.equal(
    formatRemoteRefBehavior({ refKind: 'tag', reference: 'v1.0.0', tracking: false }),
    'Pinned tag: v1.0.0'
  );
  assert.equal(
    formatRemoteRefBehavior({ refKind: 'commit', reference: 'abc123', tracking: false }),
    'Pinned commit: abc123'
  );
});

test('normalizes changed source binding without replacing current version', () => {
  const preview = normalizeRemoteSourceBindingPreview({
    skill_name: 'find-skills',
    validation: 'same_skill_changed',
    current_version: 'manual-abc',
    latest_sha: '1234567890abcdef',
    source_url: 'https://github.com/vercel-labs/skills/tree/main/skills/find-skills',
    path: 'skills/find-skills',
    ref_kind: 'branch',
    tracking: true,
    message: 'Skill names match but content differs.'
  });

  assert.equal(preview.validation, 'same_skill_changed');
  assert.equal(preview.sourceUrl, 'https://github.com/vercel-labs/skills/tree/main/skills/find-skills');
  assert.equal(preview.path, 'skills/find-skills');
  assert.equal(preview.replacesCurrent, false);
  assert.equal(preview.statusLabel, 'Source can be linked; current version will stay active.');
});

test('remote update version label handles versions while they are loading', () => {
  const label = remoteSkillUpdateVersionLabel(
    {
      currentVersion: '',
      latestSha: '',
      installedSha: ''
    },
    null
  );

  assert.equal(label, 'current unknown');
});

test('remote update summary hides successful no-change checks', () => {
  assert.equal(
    shouldShowRemoteUpdateSummary({
      state: 'up_to_date',
      updateAvailable: false,
      currentVersion: 'abc123',
      latestSha: 'abc123'
    }),
    false
  );
  assert.equal(
    shouldShowRemoteUpdateSummary({
      state: 'update_available',
      updateAvailable: true,
      currentVersion: 'abc123',
      latestSha: 'def456'
    }),
    true
  );
  assert.equal(
    shouldShowRemoteUpdateSummary({
      state: 'up_to_date',
      updateAvailable: false,
      message: 'Last check failed: timeout'
    }),
    true
  );
});

test('normalizes remote source candidates for desktop binding review', () => {
  const search = normalizeRemoteSourceCandidates({
    skill_name: 'grill-me',
    candidates: [
      {
        owner: 'santosli',
        repo: 'skills',
        path: 'remote-skills/grill-me',
        reference: 'main',
        source_url: 'https://github.com/santosli/skills/tree/main/remote-skills/grill-me',
        repo_url: 'https://github.com/santosli/skills.git',
        name: 'grill-me',
        description: 'Interview helper',
        stars: 42,
        archived: false,
        fork: false,
        updated_at: '2026-05-27T00:00:00Z',
        match_reasons: ['Exact skill name match'],
        score: 570
      }
    ]
  });

  assert.equal(search.skillName, 'grill-me');
  assert.equal(search.candidates[0].sourceUrl, 'https://github.com/santosli/skills/tree/main/remote-skills/grill-me');
  assert.equal(search.candidates[0].repoLabel, 'santosli/skills');
  assert.deepEqual(search.candidates[0].matchReasons, ['Exact skill name match']);
});

test('remote skill update summary falls back to listed current version', () => {
  assert.equal(
    remoteSkillUpdateVersionLabel(
      { currentVersion: '', installedSha: '', latestSha: '' },
      { currentVersion: 'manual-74147eb6010a' }
    ),
    'manual-74147eb6010a'
  );
  assert.equal(
    remoteSkillUpdateVersionLabel(
      { currentVersion: 'abcdef', latestSha: '123456' },
      { currentVersion: 'manual-74147eb6010a' }
    ),
    'abcdef -> 123456'
  );
  assert.equal(
    remoteSkillUpdateVersionLabel(
      {
        currentVersion: 'e4243fbf7d9398722024f62850ece90fa0d5c693',
        latestSha: 'b469d6954dd10be20d3e8d9bb59463584d42efbb'
      },
      {}
    ),
    'e4243fbf7d93 -> b469d6954dd1'
  );
});

test('remote version preview requires files before apply', () => {
  assert.equal(canApplyRemoteVersionChange({ files: [], loading: false }), false);
  assert.equal(canApplyRemoteVersionChange({ files: [{ path: 'SKILL.md' }], loading: true }), false);
  assert.equal(canApplyRemoteVersionChange({ files: [{ path: 'SKILL.md' }], loading: false }), true);
});

test('remote version preview can apply metadata-only updates', () => {
  assert.equal(
    canApplyRemoteVersionChange({
      allowNoFileChanges: true,
      files: [],
      loading: false
    }),
    true
  );
});

test('remote install warnings require explicit confirmation before apply', () => {
  const files = [{ path: 'SKILL.md' }];
  const compatibility = { status: 'warnings' };

  assert.equal(
    canApplyRemoteVersionChange({ files, compatibility, confirmWarnings: false }),
    false
  );
  assert.equal(
    canApplyRemoteVersionChange({ files, compatibility, confirmWarnings: true }),
    true
  );
  assert.equal(
    canApplyRemoteVersionChange({
      files,
      compatibility: { status: 'blocked' },
      confirmWarnings: true
    }),
    false
  );
});

test('formats operation timestamps for compact log rows', () => {
  const localTime = new Date('2026-05-27T09:08:07');
  const epochSeconds = String(Math.floor(localTime.getTime() / 1000));

  assert.equal(formatOperationTimestamp(epochSeconds), '05-27 09:08');
  assert.equal(formatOperationTimestamp('not-a-date'), 'not-a-date');
  assert.equal(formatOperationTimestamp(''), '');
});

test('normalizes remote version preview files', () => {
  const preview = normalizeRemoteVersionPreview({
    skill_name: 'demo',
    action: 'rollback',
    from_version: 'abcdef',
    to_version: 'manual-123',
    files: [{ path: 'SKILL.md', status: 'M', diff: '@@\n-old\n+new\n' }]
  });

  assert.equal(preview.skillName, 'demo');
  assert.equal(preview.files[0].label, 'Modified');
  assert.equal(remoteVersionActionLabel(preview), 'Rollback');
});

test('explains omitted remote diff previews for large files', () => {
  const preview = normalizeRemoteVersionPreview({
    files: [
      {
        path: 'SKILL.md',
        status: 'M',
        diff: '',
        old_size: 130813,
        new_size: 140901,
        old_hash: 'old-hash',
        new_hash: 'new-hash',
        too_large: true
      }
    ]
  });

  const notice = remoteDiffOmissionNotice(preview.files[0]);

  assert.equal(notice.title, 'Large file diff preview omitted');
  assert.match(notice.detail, /1 MB/);
  assert.equal(notice.sizeSummary, '128 KB -> 138 KB');
  assert.equal(notice.hashSummary, 'old-hash -> new-hash');
});

test('dashboard status notice summarizes local sync and remote checks', () => {
  const updates = normalizeRemoteSkillUpdates({
    statuses: [
      { skill_name: 'newer', state: 'update_available', update_available: true },
      { skill_name: 'fresh', state: 'up_to_date', update_available: false },
      { skill_name: 'missing', state: 'no_source', update_available: false },
      { skill_name: 'manual', state: 'not_checkable', update_available: false },
      { skill_name: 'broken', state: 'check_failed', update_available: false }
    ]
  });

  assert.equal(
    dashboardStatusNotice({ userSkillsGit: { state: 'dirty' }, remoteUpdates: updates }),
    '1 remote update available, 1 up to date, 1 check failed, 1 missing source, 1 not checkable, user skills need sync.'
  );
});
