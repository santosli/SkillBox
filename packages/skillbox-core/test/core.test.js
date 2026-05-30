import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  deploySkill,
  defaultManagedRoot,
  defaultRuntimeRoots,
  importSkill,
  parseGitHubSkillUrl,
  parseSkillMarkdown,
  readSkill,
  rollbackRemoteSkill,
  scanSkillRoots
} from '../index.js';

test('parses SKILL.md frontmatter fields', () => {
  const parsed = parseSkillMarkdown(`---
name: demo-skill
version: 1.2.3
description: "A useful demo"
metadata:
  requires:
    bins: ["demo"]
---

# Demo
`);

  assert.equal(parsed.frontmatter.name, 'demo-skill');
  assert.equal(parsed.frontmatter.version, '1.2.3');
  assert.equal(parsed.frontmatter.description, 'A useful demo');
  assert.match(parsed.body, /# Demo/);
});

test('defaults managed root to hidden ~/.skillbox directory', () => {
  const previous = process.env.SKILLBOX_HOME;
  delete process.env.SKILLBOX_HOME;

  assert.equal(path.basename(defaultManagedRoot()), '.skillbox');

  if (previous === undefined) {
    delete process.env.SKILLBOX_HOME;
  } else {
    process.env.SKILLBOX_HOME = previous;
  }
});

test('default runtime roots include Claude skills', () => {
  assert.ok(defaultRuntimeRoots().some((root) => root.endsWith(path.join('.claude', 'skills'))));
});

test('normalizes GitHub tree, blob, raw, and API URLs', () => {
  assert.deepEqual(
    parseGitHubSkillUrl('https://github.com/openai/skills/tree/main/skills/.curated/example'),
    {
      owner: 'openai',
      repo: 'skills',
      ref: 'main',
      path: 'skills/.curated/example',
      url: 'https://github.com/openai/skills/tree/main/skills/.curated/example',
      repoUrl: 'https://github.com/openai/skills.git',
      kind: 'tree'
    }
  );

  assert.equal(
    parseGitHubSkillUrl('https://github.com/acme/repo/blob/main/skills/demo/SKILL.md').path,
    'skills/demo'
  );
  assert.equal(
    parseGitHubSkillUrl('https://raw.githubusercontent.com/acme/repo/main/skills/demo/SKILL.md').path,
    'skills/demo'
  );
  assert.equal(
    parseGitHubSkillUrl('https://api.github.com/repos/acme/repo/contents/skills/demo/SKILL.md?ref=dev').ref,
    'dev'
  );
});

test('rejects GitHub URL path traversal', () => {
  assert.throws(
    () => parseGitHubSkillUrl('https://github.com/acme/repo/tree/main/skills/../../secret'),
    /path must stay inside the repository/
  );
});

test('parses GitHub tree URLs with slash refs when the skill root is known', () => {
  const source = parseGitHubSkillUrl('https://github.com/acme/repo/tree/release/1.0/skills/demo');

  assert.equal(source.ref, 'release/1.0');
  assert.equal(source.path, 'skills/demo');
});

test('scans skill directories recursively without descending into a found skill', () => {
  const root = tempDir();
  makeSkill(path.join(root, 'alpha'), 'alpha', 'Alpha skill');
  makeSkill(path.join(root, 'group', 'beta'), 'beta', 'Beta skill');

  const result = scanSkillRoots([root]);

  assert.equal(result.errors.length, 0);
  assert.deepEqual(result.skills.map((skill) => skill.name), ['alpha', 'beta']);
});

test('CLI scan is read-only and does not index runtime skills into the managed store', () => {
  const workspace = tempDir();
  const runtimeRoot = path.join(workspace, 'runtime');
  const managedRoot = path.join(workspace, 'SkillBox');
  const cliPath = fileURLToPath(new URL('../../skillbox-cli/bin/skillbox.js', import.meta.url));
  makeSkill(path.join(runtimeRoot, 'demo'), 'demo', 'Demo skill');

  const output = execFileSync(
    process.execPath,
    [cliPath, 'scan', runtimeRoot, '--managed-root', managedRoot, '--json'],
    { encoding: 'utf8' }
  );
  const result = JSON.parse(output);

  assert.equal(result.skills.length, 1);
  assert.equal(result.indexed, undefined);
  assert.equal(fs.existsSync(path.join(managedRoot, 'skillbox.sqlite')), false);
});

test('imports a user skill and deploys it as a symlink', () => {
  const workspace = tempDir();
  const source = path.join(workspace, 'source', 'demo');
  const managedRoot = path.join(workspace, 'SkillBox');
  const targetRoot = path.join(workspace, 'runtime');
  makeSkill(source, 'demo', 'Demo skill');

  const imported = importSkill({ sourceDir: source, type: 'user', managedRoot });
  const deployment = deploySkill({ skillName: 'demo', managedRoot, targetRoot });

  assert.equal(readSkill(imported.managedPath).name, 'demo');
  assert.equal(fs.lstatSync(deployment.targetPath).isSymbolicLink(), true);
  assert.equal(fs.realpathSync(deployment.targetPath), fs.realpathSync(imported.managedPath));
});

test('deploys a remote skill as a symlink to current', () => {
  const workspace = tempDir();
  const source = path.join(workspace, 'source', 'remote-demo');
  const managedRoot = path.join(workspace, 'SkillBox');
  const targetRoot = path.join(workspace, 'runtime');
  makeSkill(source, 'remote-demo', 'Remote demo skill');
  importSkill({ sourceDir: source, type: 'remote', managedRoot });

  const deployment = deploySkill({ skillName: 'remote-demo', managedRoot, targetRoot });
  const currentPath = path.join(managedRoot, 'remote-skills', 'remote-demo', 'current');

  assert.equal(fs.lstatSync(deployment.targetPath).isSymbolicLink(), true);
  assert.equal(fs.readlinkSync(deployment.targetPath), currentPath);
});

test('remote import refuses to replace a non-symlink current entry', () => {
  const workspace = tempDir();
  const source = path.join(workspace, 'source', 'remote-demo');
  const managedRoot = path.join(workspace, 'SkillBox');
  const currentPath = path.join(managedRoot, 'remote-skills', 'remote-demo', 'current');
  makeSkill(source, 'remote-demo', 'Remote demo skill');
  fs.mkdirSync(path.dirname(currentPath), { recursive: true });
  fs.writeFileSync(currentPath, 'not a symlink');

  assert.throws(
    () => importSkill({ sourceDir: source, type: 'remote', managedRoot }),
    /Refusing to replace existing non-symlink current/
  );
  assert.equal(fs.readFileSync(currentPath, 'utf8'), 'not a symlink');
});

test('rollback refuses ambiguous short SHA prefixes', () => {
  const workspace = tempDir();
  const managedRoot = path.join(workspace, 'SkillBox');
  const versionsRoot = path.join(managedRoot, 'remote-skills', 'demo', 'versions');
  makeSkill(path.join(versionsRoot, '1234aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'), 'demo', 'Demo skill');
  makeSkill(path.join(versionsRoot, '1234bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'), 'demo', 'Demo skill');

  assert.throws(
    () => rollbackRemoteSkill({ skillName: 'demo', toSha: '1234', managedRoot }),
    /Version prefix is ambiguous/
  );
});

test('import refuses symlinks that escape the source skill directory', () => {
  const workspace = tempDir();
  const source = path.join(workspace, 'source', 'demo');
  const outside = path.join(workspace, 'outside');
  const managedRoot = path.join(workspace, 'SkillBox');
  makeSkill(source, 'demo', 'Demo skill');
  fs.mkdirSync(outside, { recursive: true });
  fs.writeFileSync(path.join(outside, 'secret.txt'), 'secret');
  fs.symlinkSync(path.join(outside, 'secret.txt'), path.join(source, 'secret-link'));

  assert.throws(
    () => importSkill({ sourceDir: source, type: 'user', managedRoot }),
    /Refusing to copy symlink outside source root/
  );
});

test('import preserves internal broken symlinks', () => {
  const workspace = tempDir();
  const source = path.join(workspace, 'source', 'demo');
  const managedRoot = path.join(workspace, 'SkillBox');
  makeSkill(source, 'demo', 'Demo skill');
  fs.symlinkSync('missing.txt', path.join(source, 'missing-link'));

  const imported = importSkill({ sourceDir: source, type: 'user', managedRoot });

  const copiedLink = path.join(imported.managedPath, 'missing-link');
  assert.equal(fs.lstatSync(copiedLink).isSymbolicLink(), true);
  assert.equal(fs.readlinkSync(copiedLink), 'missing.txt');
});

test('redeploys a remote skill version symlink to current', () => {
  const workspace = tempDir();
  const source = path.join(workspace, 'source', 'remote-demo');
  const managedRoot = path.join(workspace, 'SkillBox');
  const targetRoot = path.join(workspace, 'runtime');
  const targetPath = path.join(targetRoot, 'remote-demo');
  makeSkill(source, 'remote-demo', 'Remote demo skill');
  const imported = importSkill({ sourceDir: source, type: 'remote', managedRoot });
  fs.mkdirSync(targetRoot, { recursive: true });
  fs.symlinkSync(imported.managedPath, targetPath, 'dir');

  deploySkill({ skillName: 'remote-demo', managedRoot, targetRoot });
  const currentPath = path.join(managedRoot, 'remote-skills', 'remote-demo', 'current');

  assert.equal(fs.readlinkSync(targetPath), currentPath);
});

test('refuses to overwrite an existing non-symlink deployment target', () => {
  const workspace = tempDir();
  const source = path.join(workspace, 'source', 'demo');
  const managedRoot = path.join(workspace, 'SkillBox');
  const targetRoot = path.join(workspace, 'runtime');
  makeSkill(source, 'demo', 'Demo skill');
  importSkill({ sourceDir: source, type: 'user', managedRoot });

  fs.mkdirSync(path.join(targetRoot, 'demo'), { recursive: true });

  assert.throws(
    () => deploySkill({ skillName: 'demo', managedRoot, targetRoot }),
    /Refusing to overwrite existing non-symlink target/
  );
});

function tempDir() {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'skillbox-test-'));
}

function makeSkill(dir, name, description) {
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(
    path.join(dir, 'SKILL.md'),
    `---
name: ${name}
description: "${description}"
---

# ${name}
`
  );
}
