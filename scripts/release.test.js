import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

import {
  assertReleaseAssets,
  buildLatestJson,
  extractChangelogEntry,
  insertChangelogEntry,
  normalizeVersion,
  releaseAssetName,
  releaseSeries,
  updaterBundleAssetName,
  updaterSignatureAssetName,
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
  assert.equal(updaterBundleAssetName('0.2.1'), 'SkillBox_0.2.1_universal.app.tar.gz');
  assert.equal(updaterSignatureAssetName('0.2.1'), 'SkillBox_0.2.1_universal.app.tar.gz.sig');
});

test('builds latest updater json for both universal macOS architectures', () => {
  assert.deepEqual(
    buildLatestJson({
      version: '0.3.0',
      notes: '- App auto updates.',
      pubDate: '2026-06-11T10:00:00Z',
      url: 'https://github.com/santosli/SkillBox/releases/download/v0.3.0/SkillBox_0.3.0_universal.app.tar.gz',
      signature: 'minisignature'
    }),
    {
      version: '0.3.0',
      notes: '- App auto updates.',
      pub_date: '2026-06-11T10:00:00Z',
      platforms: {
        'darwin-aarch64': {
          signature: 'minisignature',
          url: 'https://github.com/santosli/SkillBox/releases/download/v0.3.0/SkillBox_0.3.0_universal.app.tar.gz'
        },
        'darwin-x86_64': {
          signature: 'minisignature',
          url: 'https://github.com/santosli/SkillBox/releases/download/v0.3.0/SkillBox_0.3.0_universal.app.tar.gz'
        }
      }
    }
  );
});

test('release asset validation requires DMG, updater bundle, signature, and latest json', () => {
  const release = {
    assets: [
      { name: 'SkillBox_0.3.0_universal.dmg', digest: 'sha256:' + 'a'.repeat(64) },
      { name: 'SkillBox_0.3.0_universal.app.tar.gz', digest: 'sha256:' + 'b'.repeat(64) },
      { name: 'SkillBox_0.3.0_universal.app.tar.gz.sig', digest: 'sha256:' + 'c'.repeat(64) },
      { name: 'latest.json', digest: 'sha256:' + 'd'.repeat(64) }
    ]
  };

  assert.doesNotThrow(() => assertReleaseAssets(release, '0.3.0'));
  assert.throws(
    () => assertReleaseAssets({ assets: release.assets.slice(0, 3) }, '0.3.0'),
    /latest\.json/
  );
});

test('release workflow builds app and dmg bundles for updater artifacts', () => {
  const workflow = readFileSync(new URL('../.github/workflows/release.yml', import.meta.url), 'utf8');

  assert.match(workflow, /args:\s*--target universal-apple-darwin --bundles app,dmg/);
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
