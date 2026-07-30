import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import {
  canApplyUserSkillsInbound,
  canReviewUserSkillsInbound,
  inboundRelationLabel,
  normalizeUserSkillsInboundPreview,
  normalizeUserSkillsInboundStatus
} from './userSkillsInbound.js';
import {
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
  assert.deepEqual(preview.conflictAnalysis.likelyConflictFiles, ['review-docs/SKILL.md']);
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
});

test('diverged review provides only external resolution actions', () => {
  assert.match(dialogSource, /normal Git tooling outside SkillBox, then Refresh/);
  assert.match(dialogSource, /Open repository/);
  assert.match(dialogSource, /aria-label="Copy repository path"/);
  assert.match(dialogSource, /\sRefresh\s/);
  assert.doesNotMatch(dialogSource, /Keep local|Accept remote|Merge now/);
});

test('browser fixtures expose deterministic behind, dirty, diverged, and safe bootstrap states', () => {
  assert.equal(previewUserSkillsInboundStatus('behind').relation, 'behind');
  assert.equal(previewUserSkillsInbound('dirty').can_apply, false);
  assert.equal(previewUserSkillsInbound('diverged').conflict_analysis.local_only_commits, 1);
  assert.equal(previewUserSkillsInbound('remote-only').status.relation, 'remote_only');
  assert.equal(previewUserSkillsInbound('remote-only').can_apply, true);
});
