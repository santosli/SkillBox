import assert from 'node:assert/strict';
import test from 'node:test';
import { normalizeImportCandidate } from './importCandidates.js';
import {
  dashboardStatusNotice,
  normalizeRemoteSkillUpdates,
  remoteSkillRowStatus
} from './skillStatusRefresh.js';
import { normalizeUserSkillsGitStatus, userSkillRowStatus, userSyncAction } from './userSkillsGitSync.js';

test('normalizes backend is_selected false without selecting importable candidate', () => {
  const candidate = normalizeImportCandidate({
    name: 'system',
    description: 'System skill',
    source_path: '/Users/example/.codex/skills/.system/system',
    source_root: '/Users/example/.codex/skills/.system',
    suggested_type: 'remote',
    import_status: 'importable',
    is_selected: false,
    conflict: null
  });

  assert.equal(candidate.isSelected, false);
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
    repo_path: '/tmp/SkillBox/user-skills',
    remote_url: 'git@example.com:santosli/my-skills.git',
    last_error: 'push failed',
    state: 'push_failed',
    dirty: true
  });

  assert.deepEqual(status, {
    repoPath: '/tmp/SkillBox/user-skills',
    remoteUrl: 'git@example.com:santosli/my-skills.git',
    branch: '',
    dirty: true,
    rawStatus: '',
    state: 'push_failed',
    message: 'push failed'
  });
});

test('user skill row status follows shared git sync state', () => {
  assert.deepEqual(
    userSkillRowStatus({ type: 'user' }, { state: 'clean' }),
    { label: 'Synced', tone: 'green' }
  );
  assert.equal(userSkillRowStatus({ type: 'remote' }, { state: 'clean' }), null);
});

test('remote skill row status follows refreshed update state', () => {
  const updates = normalizeRemoteSkillUpdates({
    statuses: [
      {
        skill_name: 'find-skills',
        state: 'update_available',
        update_available: true,
        latest_sha: 'abc123',
        installed_sha: 'def456'
      },
      {
        skill_name: 'frontend-design',
        state: 'up_to_date',
        update_available: false
      }
    ]
  });

  assert.deepEqual(remoteSkillRowStatus({ name: 'find-skills', type: 'remote' }, updates), {
    label: 'Update available',
    tone: 'amber'
  });
  assert.deepEqual(remoteSkillRowStatus({ name: 'frontend-design', type: 'remote' }, updates), {
    label: 'Up to date',
    tone: 'green'
  });
  assert.equal(remoteSkillRowStatus({ name: 'local', type: 'user' }, updates), null);
});

test('dashboard status notice summarizes local sync and remote checks', () => {
  const updates = normalizeRemoteSkillUpdates({
    statuses: [
      { skill_name: 'newer', state: 'update_available', update_available: true },
      { skill_name: 'fresh', state: 'up_to_date', update_available: false },
      { skill_name: 'manual', state: 'not_checkable', update_available: false },
      { skill_name: 'broken', state: 'check_failed', update_available: false }
    ]
  });

  assert.equal(
    dashboardStatusNotice({ userSkillsGit: { state: 'dirty' }, remoteUpdates: updates }),
    '1 remote update available, 1 up to date, 1 check failed, 1 not checkable, user skills need sync.'
  );
});
