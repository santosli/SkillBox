import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import React from 'react';
import TestRenderer, { act } from 'react-test-renderer';
import {
  beginReviewDialogFocus,
  canApplyUserSkillsInbound,
  canReviewUserSkillsInbound,
  createInboundReviewRequestController,
  createInboundReviewRequestGate,
  handleReviewDialogKeyDown,
  InboundReviewLiveFeedback,
  inboundConflictDiagnosticGroups,
  inboundRelationLabel,
  invalidateUserSkillsInboundPreview,
  isReviewDialogFocusTarget,
  normalizeUserSkillsInboundPreview,
  normalizeUserSkillsInboundStatus,
  runInboundReviewRequest,
  useInboundReviewRequestController
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
  assert.match(appSource, /const warnings = result\.warnings \|\| \[\]/);
  assert.match(appSource, /warnings\.join\(' '\)/);
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
