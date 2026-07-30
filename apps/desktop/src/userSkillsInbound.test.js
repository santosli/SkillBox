import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import React from 'react';
import TestRenderer, { act } from 'react-test-renderer';
import { createServer } from 'vite';
import {
  appendUserSkillsInboundWarnings,
  appliedUserSkillsInboundStatus,
  beginReviewDialogFocus,
  canApplyUserSkillsInbound,
  canReviewUserSkillsInbound,
  createInboundReviewRequestController,
  createInboundReviewRequestGate,
  handleReviewDialogKeyDown,
  InboundReviewLiveFeedback,
  inboundApplyRefreshWarning,
  inboundConflictDiagnosticGroups,
  inboundRelationLabel,
  invalidateUserSkillsInboundPreview,
  isReviewDialogFocusTarget,
  normalizeUserSkillsInboundPreview,
  normalizeUserSkillsInboundStatus,
  normalizeUserSkillsInboundWarnings,
  runInboundReviewRequest,
  useInboundReviewRequestController
} from './userSkillsInbound.js';
import {
  previewSkills,
  previewUserSkillsGitChanges,
  previewUserSkillsInbound,
  previewUserSkillsInboundStatus
} from './previewData.js';

const appSource = fs.readFileSync(new URL('./App.jsx', import.meta.url), 'utf8');
const settingsSource = fs.readFileSync(
  new URL('./components/settings.jsx', import.meta.url),
  'utf8'
);
const dialogSource = fs.readFileSync(
  new URL('./components/userSkillsInbound.jsx', import.meta.url),
  'utf8'
);

function renderedText(node) {
  if (typeof node === 'string') return node;
  return (node.children || []).map(renderedText).join('');
}

function findButton(renderer, label) {
  return renderer.root
    .findAllByType('button')
    .find((button) => renderedText(button).trim() === label);
}

function createDeferred() {
  let resolve;
  let reject;
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, reject, resolve };
}

async function withProductionApp(invoke, run) {
  const previousWindow = globalThis.window;
  const previousDocument = globalThis.document;
  globalThis.window = {
    __TAURI_INTERNALS__: { invoke },
    addEventListener() {},
    clearInterval,
    clearTimeout,
    localStorage: {
      getItem() {
        return null;
      },
      removeItem() {},
      setItem() {}
    },
    location: { search: '' },
    open() {},
    removeEventListener() {},
    setInterval() {
      return 1;
    },
    setTimeout
  };
  globalThis.document = {
    activeElement: null,
    querySelector() {
      return null;
    }
  };

  const vite = await createServer({
    appType: 'custom',
    root: new URL('..', import.meta.url).pathname,
    server: { middlewareMode: true }
  });
  let renderer;

  try {
    const { default: App } = await vite.ssrLoadModule('/src/App.jsx');
    await act(async () => {
      renderer = TestRenderer.create(React.createElement(App));
    });
    await run(renderer);
  } finally {
    if (renderer) {
      await act(async () => {
        renderer.unmount();
      });
    }
    await vite.close();
    globalThis.window = previousWindow;
    globalThis.document = previousDocument;
  }
}

test('normalizes inbound status and keeps worktree state separate from relation', () => {
  const status = normalizeUserSkillsInboundStatus({
    repo_path: '/tmp/user-skills',
    worktree_state: 'dirty',
    relation: 'behind',
    ahead_count: 0,
    behind_count: 2
  });

  assert.equal(status.repoPath, '/tmp/user-skills');
  assert.equal(status.worktreeState, 'dirty');
  assert.equal(status.relation, 'behind');
  assert.equal(status.behindCount, 2);
  assert.equal(inboundRelationLabel(status), 'Behind');
});

test('review supports incoming and diverged states but apply trusts core can_apply', () => {
  assert.equal(canReviewUserSkillsInbound({ relation: 'behind', fetchError: '' }), true);
  assert.equal(canReviewUserSkillsInbound({ relation: 'diverged', fetchError: '' }), true);
  assert.equal(canReviewUserSkillsInbound({ relation: 'ahead', fetchError: '' }), false);
  assert.equal(canReviewUserSkillsInbound({ relation: 'behind', fetchError: 'auth failed' }), false);

  assert.equal(canApplyUserSkillsInbound({ canApply: true, previewId: 'preview-1' }), true);
  assert.equal(canApplyUserSkillsInbound({ canApply: false, previewId: 'preview-1' }), false);
  assert.equal(canApplyUserSkillsInbound({ canApply: true, previewId: '' }), false);
});

test('apply failure invalidation revokes the preview authorization until refresh', () => {
  const preview = { canApply: true, previewId: 'preview-1', files: [] };
  const invalidated = invalidateUserSkillsInboundPreview(preview);

  assert.equal(invalidated.canApply, false);
  assert.equal(invalidated.previewId, '');
  assert.deepEqual(invalidated.files, []);
  assert.equal(canApplyUserSkillsInbound(invalidated), false);
  assert.equal(preview.previewId, 'preview-1');
  assert.match(
    appSource,
    /preview:\s*invalidateUserSkillsInboundPreview\(current\.preview\)/
  );
  assert.match(appSource, /Refresh to review the current repository state/);
});

test('normalizes repository-wide preview and conflict diagnostics', () => {
  const preview = normalizeUserSkillsInboundPreview({
    preview_id: 'preview-1',
    can_apply: false,
    status: { relation: 'diverged', worktree_state: 'clean' },
    skill_changes: [
      {
        skill_name: 'review-docs',
        previous_name: null,
        kind: 'updated',
        files: ['review-docs/SKILL.md'],
        affected_deployments: [
          {
            target_root: '/tmp/runtime/review-docs',
            profile_id: 'codex',
            profile_name: 'Codex'
          }
        ]
      }
    ],
    conflict_analysis: {
      available: true,
      local_only_commits: 1,
      remote_only_commits: 2,
      both_changed_files: ['review-docs/SKILL.md'],
      both_changed_skills: ['review-docs'],
      likely_conflict_files: ['review-docs/SKILL.md']
    }
  });

  assert.equal(preview.previewId, 'preview-1');
  assert.equal(preview.skillChanges[0].affectedDeployments[0].profileName, 'Codex');
  assert.equal(preview.conflictAnalysis.remoteOnlyCommits, 2);
  assert.equal(preview.conflictAnalysis.available, true);
  assert.deepEqual(preview.conflictAnalysis.likelyConflictFiles, ['review-docs/SKILL.md']);
});

test('unrelated histories expose conflict analysis as unavailable instead of zero conflicts', () => {
  const preview = normalizeUserSkillsInboundPreview({
    status: { relation: 'diverged' },
    conflict_analysis: {
      available: false,
      unavailable_reason: 'Conflict analysis is unavailable because the histories have no merge base.',
      local_only_commits: 2,
      remote_only_commits: 3
    }
  });

  assert.equal(preview.conflictAnalysis.available, false);
  assert.match(preview.conflictAnalysis.unavailableReason, /no merge base/);
  assert.match(dialogSource, /Conflict analysis unavailable/);
  assert.match(dialogSource, /analysis\.unavailableReason/);
});

test('Settings exposes explicit inbound check and review without changing outbound sync', () => {
  assert.match(settingsSource, /'Checking\.\.\.' : 'Check remote'/);
  assert.match(settingsSource, /Review incoming changes/);
  assert.match(settingsSource, /canReviewUserSkillsInbound\(userSkillsInbound,\s*inboundBusy\)/);
  assert.match(appSource, /invoke\('check_user_skills_inbound'\)/);
  assert.match(appSource, /invoke\('preview_user_skills_inbound'\)/);
  assert.match(appSource, /invoke\('sync_user_skills_git'/);
  assert.match(
    appSource,
    /invoke\('sync_user_skills_git'[\s\S]*setUserSkillsGit\(normalized\);\s*setUserSkillsInbound\(null\);/
  );
});

test('inbound apply passes only structured preview authorization and keeps dirty state blocked', () => {
  assert.match(
    appSource,
    /invoke\('apply_user_skills_inbound',\s*\{\s*request:\s*\{\s*preview_id:\s*previewId,\s*actor:\s*'desktop'\s*\}/s
  );
  assert.match(dialogSource, /const canApply = canApplyUserSkillsInbound\(preview,\s*busy\)/);
  assert.match(dialogSource, /preview\?\.status\.worktreeState === 'dirty'/);
  assert.match(dialogSource, /const isDirtyBlocking = isDirty && !preview\?\.canApply/);
  assert.match(dialogSource, /const isSafeBootstrap =[\s\S]*relation === 'remote_only'[\s\S]*preview\?\.canApply/);
  assert.match(dialogSource, /Bootstrap safe/);
  assert.match(dialogSource, /Apply fast-forward/);
  assert.match(dialogSource, /aria-label="Incoming file diff"/);
  assert.match(
    appSource,
    /setUserSkillsInboundWarnings\(\(current\) =>\s*appendUserSkillsInboundWarnings\(current, result\.warnings\)/
  );
  assert.doesNotMatch(appSource, /warnings\.join\(' '\)/);
});

test('App keeps partial-success warnings visible in the real Settings workflow until dismissed', async () => {
  assert.deepEqual(
    normalizeUserSkillsInboundWarnings([' Deployment refresh was skipped. ', '', null]),
    ['Deployment refresh was skipped.']
  );
  assert.match(
    appSource,
    /setInboundReviewDialog\(\(current\) => \(\{ \.\.\.current, open: false, applying: false \}\)\);[\s\S]*appendUserSkillsInboundWarnings\(current, result\.warnings\)/
  );
  assert.match(appSource, /userSkillsInboundWarnings=\{userSkillsInboundWarnings\}/);
  assert.match(
    appSource,
    /onDismissUserSkillsInboundWarnings=\{\(\) => setUserSkillsInboundWarnings\(\[\]\)\}/
  );

  const vite = await createServer({
    appType: 'custom',
    root: new URL('..', import.meta.url).pathname,
    server: { middlewareMode: true }
  });

  try {
    const { SettingsPage } = await vite.ssrLoadModule('/src/components/settings.jsx');
    const noop = () => {};

    function Harness() {
      const [warnings, setWarnings] = React.useState([
        'Deployment refresh was skipped for Codex.'
      ]);
      const [status, setStatus] = React.useState('ready');

      return React.createElement(
        React.Fragment,
        null,
        React.createElement(SettingsPage, {
          appUpdate: { state: 'idle' },
          appUpdateInstallBlocked: false,
          doctorReport: {},
          paths: {},
          preferences: {
            remoteUpdateTimeoutSeconds: 30,
            statusRefreshIntervalMinutes: 5
          },
          status,
          usageHooks: [],
          userSkillsInbound: normalizeUserSkillsInboundStatus({
            relation: 'synced',
            worktree_state: 'clean'
          }),
          userSkillsInboundWarnings: warnings,
          userSkillsGit: {
            branch: 'main',
            remoteUrl: 'git@example.com:user/skills.git',
            repoPath: '/tmp/user-skills'
          },
          onCheckAppUpdate: noop,
          onCheckUserSkillsInbound: noop,
          onDismissUserSkillsInboundWarnings: () => setWarnings([]),
          onInstallAppUpdate: noop,
          onInstallUsageHook: noop,
          onOpenUsageHookConfig: noop,
          onRefreshUsageHooks: noop,
          onRepairStaleDeployments: noop,
          onReviewUserSkillsInbound: noop,
          onRunDoctor: noop,
          onSaveRemoteUpdateTimeout: noop,
          onSaveStatusRefreshInterval: noop,
          onSaveUserSkillsRemote: noop
        }),
        React.createElement(
          'button',
          { onClick: () => setStatus('checking_inbound'), type: 'button' },
          'Rerender Settings'
        )
      );
    }

    let renderer;
    await act(async () => {
      renderer = TestRenderer.create(React.createElement(Harness));
    });

    let alert = renderer.root.findByProps({ role: 'alert' });
    assert.equal(alert.props['aria-live'], 'assertive');
    assert.match(renderedText(alert), /Deployment refresh was skipped for Codex/);

    await act(async () => {
      renderer.root.findByProps({ children: 'Rerender Settings' }).props.onClick();
    });
    alert = renderer.root.findByProps({ role: 'alert' });
    assert.match(renderedText(alert), /Deployment refresh was skipped for Codex/);

    await act(async () => {
      renderer.root
        .findByProps({ 'aria-label': 'Dismiss incoming changes warnings' })
        .props.onClick();
    });
    assert.equal(renderer.root.findAllByProps({ role: 'alert' }).length, 0);
    renderer.unmount();
  } finally {
    await vite.close();
  }
});

test('production App commits apply success before best-effort refresh and preserves warnings until dismiss', async () => {
  assert.deepEqual(
    appendUserSkillsInboundWarnings(
      ['First complete warning.', 'Repeated warning.'],
      ['Repeated warning.', 'Second complete warning.']
    ),
    ['First complete warning.', 'Repeated warning.', 'Second complete warning.']
  );
  assert.deepEqual(
    appliedUserSkillsInboundStatus({
      new_sha: 'abc123',
      repo_path: '/tmp/user-skills'
    }),
    {
      repoPath: '/tmp/user-skills',
      branch: 'main',
      remoteUrl: '',
      worktreeState: 'clean',
      relation: 'synced',
      localSha: 'abc123',
      remoteSha: 'abc123',
      mergeBaseSha: '',
      aheadCount: 0,
      behindCount: 0,
      fetchedAt: '',
      fetchError: '',
      message: 'User skills fast-forwarded to origin/main.'
    }
  );
  assert.match(
    inboundApplyRefreshWarning([
      { label: 'Managed state refresh', error: new Error('database unavailable') }
    ]),
    /applied, but refresh failed[\s\S]*Managed state refresh: database unavailable/
  );

  const previousWindow = globalThis.window;
  const previousDocument = globalThis.document;
  let applyCount = 0;
  let inboundCheckCount = 0;
  let managedStateCount = 0;
  let rejectManagedStateRefresh;
  let rejectSecondApply;
  const managedStateRefresh = new Promise((_, reject) => {
    rejectManagedStateRefresh = reject;
  });
  const secondApply = new Promise((_, reject) => {
    rejectSecondApply = reject;
  });

  const invoke = async (command) => {
    if (command === 'managed_state') {
      managedStateCount += 1;
      if (managedStateCount > 1) {
        return managedStateRefresh;
      }
      return { is_first_use: false, paths: {}, skills: [] };
    }
    if (command === 'managed_preferences') {
      return {
        remote_update_timeout_seconds: 30,
        status_refresh_interval_minutes: 5
      };
    }
    if (command === 'user_skills_git_status') {
      return {
        branch: 'main',
        remote_url: 'git@example.com:user/skills.git',
        repo_path: '/tmp/user-skills',
        state: 'clean'
      };
    }
    if (command === 'check_user_skills_inbound') {
      inboundCheckCount += 1;
      if (inboundCheckCount === 2) {
        throw new Error('inbound refresh unavailable');
      }
      return {
        behind_count: 1,
        relation: 'behind',
        remote_sha: `remote-${inboundCheckCount}`,
        worktree_state: 'clean'
      };
    }
    if (command === 'preview_user_skills_inbound') {
      return previewUserSkillsInbound('behind');
    }
    if (command === 'apply_user_skills_inbound') {
      applyCount += 1;
      if (applyCount === 2) return secondApply;
      return {
        changed_skill_count: 1,
        new_sha: 'applied-sha-1',
        repo_path: '/tmp/user-skills',
        warnings: ['Deployment refresh was skipped for Codex.']
      };
    }
    if (
      [
        'cached_remote_skill_updates',
        'list_skill_user_metadata',
        'list_workspaces',
        'usage_hook_statuses'
      ].includes(command)
    ) {
      return command === 'cached_remote_skill_updates' ? {} : [];
    }
    if (command === 'app_update_status') {
      return { disabled: true };
    }
    return null;
  };

  globalThis.window = {
    __TAURI_INTERNALS__: { invoke },
    addEventListener() {},
    clearInterval,
    clearTimeout,
    localStorage: {
      getItem() {
        return null;
      },
      removeItem() {},
      setItem() {}
    },
    location: { search: '' },
    open() {},
    removeEventListener() {},
    setInterval() {
      return 1;
    },
    setTimeout
  };
  globalThis.document = {
    activeElement: null,
    querySelector() {
      return null;
    }
  };

  const vite = await createServer({
    appType: 'custom',
    root: new URL('..', import.meta.url).pathname,
    server: { middlewareMode: true }
  });

  try {
    const { default: App } = await vite.ssrLoadModule('/src/App.jsx');
    let renderer;
    await act(async () => {
      renderer = TestRenderer.create(React.createElement(App));
    });

    await act(async () => {
      findButton(renderer, 'Settings').props.onClick();
    });
    await act(async () => {
      await findButton(renderer, 'Check remote').props.onClick();
    });
    await act(async () => {
      await findButton(renderer, 'Review incoming changes').props.onClick();
    });

    let firstApplyRun;
    await act(async () => {
      firstApplyRun = findButton(renderer, 'Apply fast-forward').props.onClick();
      await Promise.resolve();
    });

    assert.equal(renderer.root.findAllByProps({ role: 'dialog' }).length, 0);
    let warningAlert = renderer.root.findByProps({ role: 'alert' });
    assert.match(renderedText(warningAlert), /Deployment refresh was skipped for Codex/);
    assert.doesNotMatch(renderedText(warningAlert), /refresh failed/);
    assert.match(renderedText(renderer.root), /Synced/);

    await act(async () => {
      rejectManagedStateRefresh(new Error('managed state unavailable'));
      await firstApplyRun;
    });
    warningAlert = renderer.root.findByProps({ role: 'alert' });
    assert.match(renderedText(warningAlert), /Incoming changes were applied, but refresh failed/);
    assert.match(renderedText(renderer.root), /Synced/);

    await act(async () => {
      await findButton(renderer, 'Check remote').props.onClick();
    });
    await act(async () => {
      await findButton(renderer, 'Review incoming changes').props.onClick();
    });

    let secondApplyRun;
    await act(async () => {
      secondApplyRun = findButton(renderer, 'Apply fast-forward').props.onClick();
      await Promise.resolve();
    });
    warningAlert = renderer.root.findAllByProps({ role: 'alert' }).find((alert) =>
      renderedText(alert).includes('Deployment refresh was skipped for Codex')
    );
    assert.ok(warningAlert, 'starting a later apply must not clear pending warnings');

    await act(async () => {
      rejectSecondApply(new Error('second apply rejected'));
      await secondApplyRun;
    });
    warningAlert = renderer.root.findAllByProps({ role: 'alert' }).find((alert) =>
      renderedText(alert).includes('Deployment refresh was skipped for Codex')
    );
    assert.ok(warningAlert, 'a later apply failure must preserve earlier warnings');

    await act(async () => {
      findButton(renderer, 'Dashboard').props.onClick();
    });
    assert.doesNotMatch(renderedText(renderer.root), /Deployment refresh was skipped for Codex/);
    await act(async () => {
      findButton(renderer, 'Settings').props.onClick();
    });
    warningAlert = renderer.root.findAllByProps({ role: 'alert' }).find((alert) =>
      renderedText(alert).includes('Deployment refresh was skipped for Codex')
    );
    assert.ok(warningAlert, 'navigation must not dismiss a pending Settings warning');

    await act(async () => {
      renderer.root
        .findByProps({ 'aria-label': 'Dismiss incoming changes warnings' })
        .props.onClick();
    });
    assert.equal(
      renderer.root
        .findAllByProps({ role: 'alert' })
        .filter((alert) => renderedText(alert).includes('Deployment refresh was skipped for Codex'))
        .length,
      0
    );

    assert.doesNotMatch(renderedText(renderer.root), /Deployment refresh was skipped for Codex/);
    await act(async () => {
      renderer.unmount();
    });
  } finally {
    await vite.close();
    globalThis.window = previousWindow;
    globalThis.document = previousDocument;
  }
});

test('newer outbound sync revokes a pending inbound apply refresh', async () => {
  const staleManagedRefresh = createDeferred();
  let managedStateCount = 0;
  let gitStatusCount = 0;
  let inboundCheckCount = 0;

  const invoke = async (command) => {
    if (command === 'managed_state') {
      managedStateCount += 1;
      if (managedStateCount === 2) {
        return staleManagedRefresh.promise;
      }
      return {
        is_first_use: false,
        paths: { user_skills_root: '/tmp/user-skills' },
        skills: [
          {
            name: 'generation-test',
            description: 'Generation test skill',
            deployments: [],
            path: '/tmp/user-skills/generation-test',
            skill_md_path: '/tmp/user-skills/generation-test/SKILL.md',
            source_root: '/tmp/user-skills',
            status: 'synced',
            type: 'user'
          }
        ]
      };
    }
    if (command === 'managed_preferences') {
      return {
        remote_update_timeout_seconds: 30,
        status_refresh_interval_minutes: 5
      };
    }
    if (command === 'user_skills_git_status') {
      gitStatusCount += 1;
      return gitStatusCount === 1
        ? {
            branch: 'main',
            remote_url: 'git@example.com:initial/skills.git',
            repo_path: '/tmp/user-skills',
            state: 'clean'
          }
        : {
            branch: 'main',
            remote_url: 'git@example.com:stale-refresh/skills.git',
            repo_path: '/tmp/stale-refresh',
            state: 'dirty'
          };
    }
    if (command === 'check_user_skills_inbound') {
      inboundCheckCount += 1;
      return {
        behind_count: 1,
        relation: 'behind',
        remote_sha: `remote-${inboundCheckCount}`,
        worktree_state: 'clean'
      };
    }
    if (command === 'preview_user_skills_inbound') {
      return previewUserSkillsInbound('behind');
    }
    if (command === 'apply_user_skills_inbound') {
      return {
        changed_skill_count: 1,
        new_sha: 'applied-sha',
        repo_path: '/tmp/user-skills',
        warnings: []
      };
    }
    if (command === 'user_skills_git_changes') {
      return previewUserSkillsGitChanges();
    }
    if (command === 'list_import_records') {
      return { records: [] };
    }
    if (command === 'list_user_skill_versions') {
      return { current_version: '', versions: [] };
    }
    if (command === 'sync_user_skills_git') {
      return {
        branch: 'main',
        remote_url: 'git@example.com:outbound/skills.git',
        repo_path: '/tmp/outbound',
        state: 'clean',
        message: 'Outbound sync complete.'
      };
    }
    if (
      [
        'cached_remote_skill_updates',
        'list_skill_user_metadata',
        'list_workspaces',
        'usage_hook_statuses'
      ].includes(command)
    ) {
      return command === 'cached_remote_skill_updates' ? {} : [];
    }
    if (command === 'app_update_status') {
      return { disabled: true };
    }
    return null;
  };

  await withProductionApp(invoke, async (renderer) => {
    await act(async () => {
      findButton(renderer, 'Settings').props.onClick();
    });
    await act(async () => {
      await findButton(renderer, 'Check remote').props.onClick();
    });
    await act(async () => {
      await findButton(renderer, 'Review incoming changes').props.onClick();
    });

    let applyRun;
    await act(async () => {
      applyRun = findButton(renderer, 'Apply fast-forward').props.onClick();
      await Promise.resolve();
    });
    assert.equal(managedStateCount, 2);

    await act(async () => {
      findButton(renderer, 'Dashboard').props.onClick();
    });
    await act(async () => {
      renderer.root.findByProps({ className: 'skillCardHitArea' }).props.onClick();
    });
    await act(async () => {
      await findButton(renderer, 'Sync now').props.onClick();
    });
    await act(async () => {
      await renderer.root.findByProps({ className: 'gitCommitForm' }).props.onSubmit({
        preventDefault() {}
      });
    });

    staleManagedRefresh.resolve({
      is_first_use: false,
      paths: { user_skills_root: '/tmp/stale-refresh' },
      skills: []
    });
    await act(async () => {
      await applyRun;
    });
    await act(async () => {
      findButton(renderer, 'Settings').props.onClick();
    });

    assert.match(renderedText(renderer.root), /\/tmp\/outbound/);
    assert.doesNotMatch(renderedText(renderer.root), /\/tmp\/stale-refresh/);
    assert.doesNotMatch(renderedText(renderer.root), /Behind/);
  });
});

test('newer remote save revokes an older inbound check result', async () => {
  const staleInboundCheck = createDeferred();
  let inboundCheckCount = 0;

  const invoke = async (command) => {
    if (command === 'managed_state') {
      return {
        is_first_use: false,
        paths: { user_skills_root: '/tmp/user-skills' },
        skills: []
      };
    }
    if (command === 'managed_preferences') {
      return {
        remote_update_timeout_seconds: 30,
        status_refresh_interval_minutes: 5
      };
    }
    if (command === 'user_skills_git_status') {
      return {
        branch: 'main',
        remote_url: 'git@example.com:initial/skills.git',
        repo_path: '/tmp/user-skills',
        state: 'clean'
      };
    }
    if (command === 'check_user_skills_inbound') {
      inboundCheckCount += 1;
      return staleInboundCheck.promise;
    }
    if (command === 'set_user_skills_git_remote') {
      return {
        branch: 'main',
        remote_url: 'git@example.com:saved/skills.git',
        repo_path: '/tmp/user-skills',
        state: 'clean'
      };
    }
    if (
      [
        'cached_remote_skill_updates',
        'list_skill_user_metadata',
        'list_workspaces',
        'usage_hook_statuses'
      ].includes(command)
    ) {
      return command === 'cached_remote_skill_updates' ? {} : [];
    }
    if (command === 'app_update_status') {
      return { disabled: true };
    }
    return null;
  };

  await withProductionApp(invoke, async (renderer) => {
    await act(async () => {
      findButton(renderer, 'Settings').props.onClick();
    });

    const checkRemote = findButton(renderer, 'Check remote');
    const remoteInput = renderer.root.findByProps({
      placeholder: 'git@github.com:santosli/user-skills.git'
    });
    const remoteForm = remoteInput.parent.parent;

    await act(async () => {
      remoteInput.props.onChange({ target: { value: 'git@example.com:saved/skills.git' } });
    });

    let checkRun;
    let saveRun;
    await act(async () => {
      checkRun = checkRemote.props.onClick();
      saveRun = remoteForm.props.onSubmit({ preventDefault() {} });
      await saveRun;
    });

    staleInboundCheck.resolve({
      behind_count: 4,
      relation: 'behind',
      remote_sha: 'stale-remote-sha',
      worktree_state: 'clean'
    });
    await act(async () => {
      await checkRun;
    });

    assert.equal(inboundCheckCount, 1);
    assert.equal(
      renderer.root.findByProps({
        placeholder: 'git@github.com:santosli/user-skills.git'
      }).props.value,
      'git@example.com:saved/skills.git'
    );
    assert.doesNotMatch(renderedText(renderer.root), /Behind/);
  });
});

test('refresh claims its generation before paint and cannot overwrite a newer remote save', async () => {
  let managedStateCount = 0;
  const invoke = async (command) => {
    if (command === 'managed_state') {
      managedStateCount += 1;
      return {
        is_first_use: false,
        paths: {
          user_skills_root:
            managedStateCount === 1 ? '/tmp/user-skills' : '/tmp/stale-status-refresh'
        },
        skills: []
      };
    }
    if (command === 'managed_preferences') {
      return {
        remote_update_timeout_seconds: 30,
        status_refresh_interval_minutes: 5
      };
    }
    if (command === 'user_skills_git_status') {
      return {
        branch: 'main',
        remote_url: 'git@example.com:stale/skills.git',
        repo_path: '/tmp/stale-status-refresh',
        state: 'dirty'
      };
    }
    if (command === 'set_user_skills_git_remote') {
      return {
        branch: 'main',
        remote_url: 'git@example.com:saved/skills.git',
        repo_path: '/tmp/user-skills',
        state: 'clean'
      };
    }
    if (command === 'check_remote_skill_updates') {
      return { statuses: [] };
    }
    if (
      [
        'cached_remote_skill_updates',
        'list_skill_user_metadata',
        'list_workspaces',
        'usage_hook_statuses'
      ].includes(command)
    ) {
      return command === 'cached_remote_skill_updates' ? {} : [];
    }
    if (command === 'app_update_status') {
      return { disabled: true };
    }
    return null;
  };

  const previousRequestAnimationFrame = globalThis.requestAnimationFrame;
  const frames = [];
  try {
    await withProductionApp(invoke, async (renderer) => {
      globalThis.requestAnimationFrame = (callback) => {
        frames.push(callback);
        return frames.length;
      };

      let refreshRun;
      await act(async () => {
        refreshRun = findButton(renderer, 'Refresh').props.onClick();
        await Promise.resolve();
      });
      assert.equal(frames.length, 1);
      await act(async () => {
        frames.shift()();
        await Promise.resolve();
      });
      assert.equal(frames.length, 1);

      await act(async () => {
        findButton(renderer, 'Settings').props.onClick();
      });
      const remoteInput = renderer.root.findByProps({
        placeholder: 'git@github.com:santosli/user-skills.git'
      });
      const remoteForm = remoteInput.parent.parent;
      await act(async () => {
        remoteInput.props.onChange({ target: { value: 'git@example.com:saved/skills.git' } });
      });
      await act(async () => {
        await remoteForm.props.onSubmit({ preventDefault() {} });
      });
      await act(async () => {
        frames.shift()();
        await refreshRun;
      });

      assert.equal(managedStateCount, 1, 'stale refresh must stop before invoking backend work');
      assert.equal(
        renderer.root.findByProps({
          placeholder: 'git@github.com:santosli/user-skills.git'
        }).props.value,
        'git@example.com:saved/skills.git'
      );
      assert.doesNotMatch(renderedText(renderer.root), /stale-status-refresh/);
    });
  } finally {
    globalThis.requestAnimationFrame = previousRequestAnimationFrame;
  }
});

test('single-skill refresh success and error cannot overwrite a newer remote save', async () => {
  const staleChecks = [createDeferred(), createDeferred()];
  let checkCount = 0;
  const invoke = async (command) => {
    if (command === 'managed_state') {
      return {
        is_first_use: false,
        paths: { user_skills_root: '/tmp/user-skills' },
        skills: [previewSkills.find((skill) => skill.type === 'remote')]
      };
    }
    if (command === 'managed_preferences') {
      return {
        remote_update_timeout_seconds: 30,
        status_refresh_interval_minutes: 5
      };
    }
    if (command === 'user_skills_git_status') {
      return {
        branch: 'main',
        remote_url: 'git@example.com:initial/skills.git',
        repo_path: '/tmp/user-skills',
        state: 'clean'
      };
    }
    if (command === 'check_remote_skill_update') {
      const deferred = staleChecks[checkCount];
      checkCount += 1;
      return deferred.promise;
    }
    if (command === 'set_user_skills_git_remote') {
      return {
        branch: 'main',
        remote_url: `git@example.com:saved-${checkCount}/skills.git`,
        repo_path: '/tmp/user-skills',
        state: 'clean'
      };
    }
    if (command === 'list_remote_skill_versions') {
      return { current_version: '', skill_name: 'docs-reviewer', versions: [] };
    }
    if (command === 'list_operations') {
      return { operations: [] };
    }
    if (command === 'list_import_records') {
      return { records: [] };
    }
    if (
      [
        'cached_remote_skill_updates',
        'list_skill_user_metadata',
        'list_workspaces',
        'usage_hook_statuses'
      ].includes(command)
    ) {
      return command === 'cached_remote_skill_updates'
        ? {
            statuses: [
              {
                skill_name: 'docs-reviewer',
                source_type: 'github',
                source_url: 'https://github.com/acme/docs-reviewer',
                state: 'up_to_date'
              }
            ]
          }
        : [];
    }
    if (command === 'app_update_status') {
      return { disabled: true };
    }
    return null;
  };

  await withProductionApp(invoke, async (renderer) => {
    for (let attempt = 0; attempt < 10; attempt += 1) {
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 0));
      });
      if (renderedText(renderer.root).includes('Up to date')) {
        break;
      }
    }
    for (const outcome of ['success', 'error']) {
      await act(async () => {
        findButton(renderer, 'Dashboard').props.onClick();
      });
      await act(async () => {
        renderer.root.findByProps({ className: 'skillCardHitArea' }).props.onClick();
        await Promise.resolve();
        await Promise.resolve();
      });
      let checkRun;
      await act(async () => {
        const checkUpdate = findButton(renderer, 'Check update');
        assert.ok(
          checkUpdate,
          `initial App refresh should settle before checking one skill; checks=${checkCount}; buttons=${renderer.root
            .findAllByType('button')
            .map((button) => renderedText(button).trim())
            .join(' | ')}`
        );
        checkRun = checkUpdate.props.onClick();
        await Promise.resolve();
      });
      await act(async () => {
        findButton(renderer, 'Settings').props.onClick();
      });
      const remoteInput = renderer.root.findByProps({
        placeholder: 'git@github.com:santosli/user-skills.git'
      });
      const remoteForm = remoteInput.parent.parent;
      const savedRemote = `git@example.com:saved-${checkCount}/skills.git`;
      await act(async () => {
        remoteInput.props.onChange({ target: { value: savedRemote } });
      });
      await act(async () => {
        await remoteForm.props.onSubmit({ preventDefault() {} });
      });
      await act(async () => {
        if (outcome === 'success') {
          staleChecks[checkCount - 1].resolve({
            checked_at: new Date().toISOString(),
            statuses: [
              {
                skill_name: 'remote-demo',
                state: 'update_available',
                message: 'stale single-skill success'
              }
            ]
          });
        } else {
          staleChecks[checkCount - 1].reject(new Error('stale single-skill error'));
        }
        await checkRun;
      });
      assert.equal(
        renderer.root.findByProps({
          placeholder: 'git@github.com:santosli/user-skills.git'
        }).props.value,
        savedRemote
      );
      assert.doesNotMatch(renderedText(renderer.root), /stale single-skill/);
    }
  });
});

test('diverged review provides only external resolution actions', () => {
  assert.match(dialogSource, /normal Git tooling outside SkillBox, then Refresh/);
  assert.match(dialogSource, /Open repository/);
  assert.match(dialogSource, /aria-label="Copy repository path"/);
  assert.match(dialogSource, /\sRefresh\s/);
  assert.doesNotMatch(dialogSource, /Keep local|Accept remote|Merge now/);
});

test('diverged review exposes bounded expandable conflict lists', () => {
  const groups = inboundConflictDiagnosticGroups({
    bothChangedSkills: ['demo'],
    bothChangedFiles: ['demo/removed.md'],
    likelyConflictFiles: []
  });
  assert.deepEqual(groups[0], {
    id: 'both-changed-skills',
    label: 'Skills changed on both sides',
    items: ['demo']
  });
  assert.deepEqual(groups[1], {
    id: 'both-changed-files',
    label: 'Files changed on both sides',
    items: ['demo/removed.md']
  });
  assert.deepEqual(groups[2], {
    id: 'likely-conflicts',
    label: 'Likely conflict files',
    items: []
  });
  assert.notStrictEqual(groups[1].items, groups[2].items);
  assert.match(dialogSource, /inboundConflictDiagnosticGroups\(analysis\)/);
  assert.match(dialogSource, /<details key=\{id\}>/);
  assert.match(dialogSource, /<summary>/);
  assert.match(dialogSource, /items\.slice\(0, 8\)\.map/);
  assert.match(dialogSource, /Showing 8 of \{items\.length\} items/);
});

test('review dialog manages initial focus, Escape, focus trap, and focus restore', () => {
  const ownerDocument = {
    activeElement: null,
    defaultView: {
      getComputedStyle: () => ({ display: 'block', visibility: 'visible' })
    }
  };
  const focusable = (name) => ({
    name,
    isConnected: true,
    disabled: false,
    hidden: false,
    ownerDocument,
    getAttribute: () => null,
    getClientRects: () => [{}],
    focus() {
      ownerDocument.activeElement = this;
    }
  });
  const restore = focusable('restore');
  const first = focusable('first');
  const last = focusable('last');
  const dialogElement = {
    ownerDocument,
    contains: (element) => [first, last].includes(element),
    querySelectorAll: () => [first, last]
  };
  const event = (key, shiftKey = false) => ({
    key,
    shiftKey,
    prevented: false,
    stopped: false,
    preventDefault() {
      this.prevented = true;
    },
    stopPropagation() {
      this.stopped = true;
    }
  });

  const cleanup = beginReviewDialogFocus(first, restore);
  assert.equal(ownerDocument.activeElement, first);

  ownerDocument.activeElement = last;
  const forwardTab = event('Tab');
  handleReviewDialogKeyDown(forwardTab, { dialogElement, onClose() {} });
  assert.equal(forwardTab.prevented, true);
  assert.equal(ownerDocument.activeElement, first);

  const backwardTab = event('Tab', true);
  handleReviewDialogKeyDown(backwardTab, { dialogElement, onClose() {} });
  assert.equal(backwardTab.prevented, true);
  assert.equal(ownerDocument.activeElement, last);

  let closeCount = 0;
  const escape = event('Escape');
  handleReviewDialogKeyDown(escape, {
    dialogElement,
    onClose() {
      closeCount += 1;
    }
  });
  assert.equal(closeCount, 1);
  assert.equal(escape.prevented, true);
  assert.equal(escape.stopped, true);

  const blockedEscape = event('Escape');
  handleReviewDialogKeyDown(blockedEscape, {
    dialogElement,
    closeDisabled: true,
    onClose() {
      closeCount += 1;
    }
  });
  assert.equal(closeCount, 1);
  assert.equal(blockedEscape.prevented, true);
  assert.equal(blockedEscape.stopped, true);

  cleanup();
  assert.equal(ownerDocument.activeElement, restore);
  assert.match(dialogSource, /ref=\{closeButtonRef\}/);
  assert.match(
    dialogSource,
    /aria-current=\{activeFile\?\.path === file\.path \? 'true' : undefined\}/
  );
  assert.match(dialogSource, /aria-pressed=\{activeFile\?\.path === file\.path\}/);
});

test('Cancel and Escape invalidate late inbound preview success and error', async () => {
  const deferred = () => {
    let resolve;
    let reject;
    const promise = new Promise((resolvePromise, rejectPromise) => {
      resolve = resolvePromise;
      reject = rejectPromise;
    });
    return { promise, reject, resolve };
  };
  const event = (key) => ({
    key,
    preventDefault() {},
    stopPropagation() {}
  });
  for (const closeMethod of ['Cancel', 'Escape']) {
    for (const outcome of ['success', 'error']) {
      const gate = createInboundReviewRequestGate();
      const mutations = [];
      const pending = deferred();
      const request = gate.begin();
      const run = runInboundReviewRequest({
        gate,
        requestGeneration: request,
        loadPreview: () => pending.promise,
        onSuccess: (result) => mutations.push(`success:${result}`),
        onError: (error) => mutations.push(`error:${error.message}`)
      });

      if (closeMethod === 'Escape') {
        handleReviewDialogKeyDown(event('Escape'), {
          dialogElement: null,
          onClose() {
            gate.cancel();
          }
        });
      } else {
        gate.cancel();
      }

      if (outcome === 'success') {
        pending.resolve('preview');
      } else {
        pending.reject(new Error('network failed'));
      }
      await run;
      assert.deepEqual(mutations, [], `${closeMethod} must ignore late ${outcome}`);
    }
  }

  assert.match(appSource, /await inboundReviewRequestControllerRef\.current\.run\(\{/);
  assert.match(appSource, /inboundReviewRequestControllerRef\.current\.cancel\(\)/);
});

test('disposing the production inbound review controller cancels pending App work', async () => {
  const deferred = () => {
    let resolve;
    let reject;
    const promise = new Promise((resolvePromise, rejectPromise) => {
      resolve = resolvePromise;
      reject = rejectPromise;
    });
    return { promise, reject, resolve };
  };

  for (const outcome of ['success', 'error']) {
    const controller = createInboundReviewRequestController();
    const pending = deferred();
    const mutations = [];
    const run = controller.run({
      loadPreview: () => pending.promise,
      onSuccess: (result) => mutations.push(`success:${result}`),
      onError: (error) => mutations.push(`error:${error.message}`)
    });

    controller.dispose();
    if (outcome === 'success') {
      pending.resolve('preview');
    } else {
      pending.reject(new Error('network failed'));
    }

    assert.equal(await run, false);
    assert.deepEqual(mutations, []);

    let loadedAfterDispose = false;
    assert.equal(
      await controller.run({
        loadPreview: async () => {
          loadedAfterDispose = true;
          return 'late';
        },
        onSuccess() {},
        onError() {}
      }),
      false
    );
    assert.equal(loadedAfterDispose, false);
  }

});

test('react-test-renderer StrictMode replay installs a live controller and unmount cancels late work', async () => {
  const deferred = () => {
    let resolve;
    let reject;
    const promise = new Promise((resolvePromise, rejectPromise) => {
      resolve = resolvePromise;
      reject = rejectPromise;
    });
    return { promise, reject, resolve };
  };

  for (const lateOutcome of ['success', 'error']) {
    const pendingPreviews = [];
    const mutations = [];
    let controllerRef;

    function Harness() {
      controllerRef = useInboundReviewRequestController();
      React.useEffect(() => {
        const pending = deferred();
        pendingPreviews.push(pending);
        controllerRef.current.run({
          loadPreview: () => pending.promise,
          onSuccess: (result) => mutations.push(`success:${result}`),
          onError: (error) => mutations.push(`error:${error.message}`)
        });
      }, [controllerRef]);
      return null;
    }

    let renderer;
    await act(async () => {
      renderer = TestRenderer.create(
        React.createElement(React.StrictMode, null, React.createElement(Harness))
      );
    });

    // React 18's test renderer does not automatically replay passive effects,
    // so invoke its recorded StrictMode cleanup/setup cycle on the mounted fiber.
    const harnessFiber = renderer.root.findByType(Harness)._fiber;
    const lastEffect = harnessFiber.updateQueue.lastEffect;
    const effects = [];
    let effect = lastEffect.next;
    do {
      effects.push(effect);
      effect = effect.next;
    } while (effect !== lastEffect.next);

    await act(async () => {
      for (const passiveEffect of effects) {
        passiveEffect.destroy?.();
      }
      for (const passiveEffect of effects) {
        passiveEffect.destroy = passiveEffect.create?.();
      }
    });

    assert.equal(pendingPreviews.length, 2);
    await act(async () => {
      pendingPreviews[0].resolve('stale preview');
      pendingPreviews[1].resolve('replayed preview');
      await Promise.all(pendingPreviews.map((pending) => pending.promise));
    });
    assert.deepEqual(mutations, ['success:replayed preview']);

    const late = deferred();
    const lateRun = controllerRef.current.run({
      loadPreview: () => late.promise,
      onSuccess: (result) => mutations.push(`success:${result}`),
      onError: (error) => mutations.push(`error:${error.message}`)
    });

    await act(async () => {
      renderer.unmount();
    });
    if (lateOutcome === 'success') {
      late.resolve('after unmount');
    } else {
      late.reject(new Error('after unmount'));
    }
    assert.equal(await lateRun, false);
    assert.deepEqual(mutations, ['success:replayed preview']);
  }

  assert.match(appSource, /useInboundReviewRequestController\(\)/);
});

test('inbound preview and apply states expose live accessibility feedback', async () => {
  assert.match(
    dialogSource,
    /<InboundReviewLiveFeedback\s+applying=\{dialog\.applying\}\s+error=\{dialog\.error\}\s+loading=\{dialog\.loading\}/
  );

  function Harness() {
    const [feedback, setFeedback] = React.useState({
      loading: true,
      applying: false,
      error: ''
    });
    return React.createElement(
      React.Fragment,
      null,
      React.createElement(InboundReviewLiveFeedback, feedback),
      React.createElement(
        'button',
        {
          onClick: () =>
            setFeedback({ loading: false, applying: true, error: '' })
        },
        'Apply'
      ),
      React.createElement(
        'button',
        {
          onClick: () =>
            setFeedback({
              loading: false,
              applying: false,
              error: 'Unable to apply incoming changes.'
            })
        },
        'Fail'
      )
    );
  }

  let renderer;
  await act(async () => {
    renderer = TestRenderer.create(React.createElement(Harness));
  });
  const previewStatus = renderer.root.findByProps({ role: 'status' });
  assert.equal(previewStatus.props['aria-live'], 'polite');
  assert.equal(previewStatus.children.join(''), 'Checking remote repository...');

  const [applyButton, failButton] = renderer.root.findAllByType('button');
  await act(async () => {
    applyButton.props.onClick();
  });
  const applyStatus = renderer.root.findByProps({ role: 'status' });
  assert.equal(applyStatus.children.join(''), 'Applying incoming changes...');

  await act(async () => {
    failButton.props.onClick();
  });
  const alert = renderer.root.findByProps({ role: 'alert' });
  assert.equal(alert.props['aria-live'], 'assertive');
  assert.equal(alert.children.join(''), 'Unable to apply incoming changes.');
  renderer.unmount();
});

test('focus restore skips stale triggers and uses a visible stable fallback', () => {
  const ownerDocument = {
    activeElement: null,
    defaultView: {
      getComputedStyle: (element) => ({
        display: element.display || 'block',
        visibility: element.visibility || 'visible'
      })
    }
  };
  const target = (name, overrides = {}) => ({
    name,
    isConnected: true,
    disabled: false,
    hidden: false,
    ownerDocument,
    getAttribute: () => null,
    getClientRects: () => [{}],
    focus() {
      ownerDocument.activeElement = this;
    },
    ...overrides
  });

  const initial = target('close');
  const fallback = target('content');
  for (const staleTrigger of [
    target('detached', { isConnected: false }),
    target('disabled', { disabled: true }),
    target('hidden', { getClientRects: () => [] }),
    target('aria-disabled', {
      getAttribute: (name) => (name === 'aria-disabled' ? 'true' : null)
    })
  ]) {
    const cleanup = beginReviewDialogFocus(initial, staleTrigger, fallback);
    cleanup();
    assert.equal(ownerDocument.activeElement, fallback);
  }

  assert.equal(isReviewDialogFocusTarget(target('visible')), true);
  assert.equal(isReviewDialogFocusTarget(target('display-none', { display: 'none' })), false);
  assert.match(appSource, /restoreFocusFallback=\{contentRef\.current\}/);
  assert.match(appSource, /className="content" ref=\{contentRef\} tabIndex=\{-1\}/);
});

test('Save remote and inbound operations disable each other', () => {
  assert.match(
    settingsSource,
    /'checking_inbound',[\s\S]*'previewing_inbound',[\s\S]*'applying_inbound'/
  );
  assert.match(settingsSource, /const inboundBusy = inboundOperationBusy \|\| remoteSaveBusy/);
  assert.match(settingsSource, /if \(inboundOperationBusy \|\| remoteSaveBusy\) return/);
  assert.match(
    settingsSource,
    /disabled=\{status === 'syncing' \|\| inboundOperationBusy \|\| remoteSaveBusy\}/
  );
});

test('browser fixtures expose deterministic behind, dirty, diverged, and safe bootstrap states', () => {
  assert.equal(previewUserSkillsInboundStatus('behind').relation, 'behind');
  assert.equal(previewUserSkillsInbound('dirty').can_apply, false);
  assert.equal(previewUserSkillsInbound('diverged').conflict_analysis.local_only_commits, 1);
  assert.equal(previewUserSkillsInbound('remote-only').status.relation, 'remote_only');
  assert.equal(previewUserSkillsInbound('remote-only').can_apply, true);
});
