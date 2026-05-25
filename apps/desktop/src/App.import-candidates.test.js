import assert from 'node:assert/strict';
import test from 'node:test';
import { normalizeImportCandidate } from './importCandidates.js';
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
