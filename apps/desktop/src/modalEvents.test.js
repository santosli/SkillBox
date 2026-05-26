import assert from 'node:assert/strict';
import test from 'node:test';

import { closeOnBackdropClick } from './modalEvents.js';

test('closes modal when the backdrop itself is clicked', () => {
  const backdrop = {};
  let closeCount = 0;

  closeOnBackdropClick(
    { target: backdrop, currentTarget: backdrop },
    () => {
      closeCount += 1;
    }
  );

  assert.equal(closeCount, 1);
});

test('keeps modal open when a child inside the dialog is clicked', () => {
  const backdrop = {};
  const dialog = {};
  let closeCount = 0;

  closeOnBackdropClick(
    { target: dialog, currentTarget: backdrop },
    () => {
      closeCount += 1;
    }
  );

  assert.equal(closeCount, 0);
});
