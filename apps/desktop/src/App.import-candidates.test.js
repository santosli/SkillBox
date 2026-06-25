import assert from 'node:assert/strict';
import test from 'node:test';
import { normalizeImportCandidate } from './importCandidates.js';
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
  waitForNextPaint,
  userSkillsSyncProgressSteps,
  userSkillRowStatus,
  userSyncAction
} from './userSkillsGitSync.js';

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

test('user sync action is setup before remote and hidden for remote skills', () => {
  assert.equal(userSyncAction({ state: 'not_configured' }, 'user'), 'Set up sync');
  assert.equal(userSyncAction({ state: 'clean' }, 'remote'), null);
});

test('user sync action retries failed push and syncs configured remotes', () => {
  assert.equal(userSyncAction({ state: 'push_failed' }, 'user'), 'Retry push');
  assert.equal(userSyncAction({ state: 'dirty' }, 'user'), 'Sync now');
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
