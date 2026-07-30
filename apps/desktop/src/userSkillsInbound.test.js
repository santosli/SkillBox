import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import {
  beginReviewDialogFocus,
  canApplyUserSkillsInbound,
  canReviewUserSkillsInbound,
  handleReviewDialogKeyDown,
  inboundRelationLabel,
  invalidateUserSkillsInboundPreview,
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

test('diverged review exposes bounded expandable conflict lists', () => {
  assert.match(dialogSource, /\['Skills changed by both', analysis\?\.bothChangedSkills \|\| \[\]\]/);
  assert.match(dialogSource, /\['Files changed by both', analysis\?\.bothChangedFiles \|\| \[\]\]/);
  assert.match(dialogSource, /\['Likely conflict files', analysis\?\.likelyConflictFiles \|\| \[\]\]/);
  assert.match(dialogSource, /<details key=\{label\}>/);
  assert.match(dialogSource, /<summary>/);
  assert.match(dialogSource, /items\.slice\(0, 8\)\.map/);
  assert.match(dialogSource, /Showing 8 of \{items\.length\} items/);
});

test('review dialog manages initial focus, Escape, focus trap, and focus restore', () => {
  const ownerDocument = { activeElement: null };
  const focusable = (name) => ({
    name,
    isConnected: true,
    getAttribute: () => null,
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
