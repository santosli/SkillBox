import assert from 'node:assert/strict';
import test from 'node:test';

import {
  extractChangelogEntry,
  insertChangelogEntry,
  normalizeVersion,
  releaseAssetName,
  releaseSeries,
  updateCaskContent,
  updateIssueTemplateVersionPlaceholder,
  updateSecuritySupport
} from './release.js';

test('normalizes semantic release versions', () => {
  assert.equal(normalizeVersion('v0.2.1'), '0.2.1');
  assert.equal(normalizeVersion('0.3.0-alpha.1'), '0.3.0-alpha.1');
  assert.throws(() => normalizeVersion('0.2'), /semantic/);
});

test('derives release labels and assets', () => {
  assert.equal(releaseSeries('0.2.1'), '0.2.x');
  assert.equal(releaseAssetName('0.2.1'), 'SkillBox_0.2.1_universal.dmg');
});

test('inserts and extracts changelog release notes', () => {
  const changelog = [
    '# Changelog',
    '',
    '## Unreleased',
    '',
    '- No unreleased changes.',
    '',
    '## 0.2.0',
    '',
    '- Previous release.',
    ''
  ].join('\n');

  const updated = insertChangelogEntry(changelog, '0.2.1', '- Main change.\n- Another change.');
  assert.match(updated, /## 0\.2\.1\n\n- Main change\.\n- Another change\.\n\n## 0\.2\.0/);
  assert.equal(extractChangelogEntry(updated, '0.2.1'), '- Main change.\n- Another change.');
});

test('updates supported security series', () => {
  const security = [
    '| Version | Supported |',
    '| --- | --- |',
    '| `0.2.x` | Yes |',
    '| `0.1.x` | No |'
  ].join('\n');

  assert.equal(
    updateSecuritySupport(security, '0.3.0'),
    [
      '| Version | Supported |',
      '| --- | --- |',
      '| `0.3.x` | Yes |',
      '| `0.2.x` | No |',
      '| `0.1.x` | No |'
    ].join('\n')
  );
});

test('updates cask version checksum and canonical URLs', () => {
  const cask = [
    'cask "skillbox" do',
    '  version "0.1.1"',
    '  sha256 "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"',
    '',
    '  url "https://github.com/santosli/skill-box/releases/download/v#{version}/SkillBox_#{version}_universal.dmg"',
    '  homepage "https://github.com/santosli/skill-box"',
    'end',
    ''
  ].join('\n');

  const updated = updateCaskContent(
    cask,
    '0.2.1',
    'sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'
  );
  assert.match(updated, /version "0\.2\.1"/);
  assert.match(updated, /sha256 "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"/);
  assert.match(updated, /github\.com\/santosli\/SkillBox\/releases/);
  assert.match(updated, /homepage "https:\/\/github\.com\/santosli\/SkillBox"/);
});

test('updates only the SkillBox version placeholder in issue templates', () => {
  const template = [
    'attributes:',
    '  label: Install method',
    '  placeholder: "GitHub Release DMG or Homebrew cask"',
    'attributes:',
    '  label: SkillBox version',
    '  placeholder: "0.2.0"',
    ''
  ].join('\n');

  const updated = updateIssueTemplateVersionPlaceholder(template, '0.2.1');
  assert.match(updated, /placeholder: "GitHub Release DMG or Homebrew cask"/);
  assert.match(updated, /label: SkillBox version\n  placeholder: "0\.2\.1"/);
});
