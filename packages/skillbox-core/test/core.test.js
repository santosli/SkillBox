import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  deploySkill,
  importSkill,
  parseGitHubSkillUrl,
  parseSkillMarkdown,
  readSkill,
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

test('scans skill directories recursively without descending into a found skill', () => {
  const root = tempDir();
  makeSkill(path.join(root, 'alpha'), 'alpha', 'Alpha skill');
  makeSkill(path.join(root, 'group', 'beta'), 'beta', 'Beta skill');

  const result = scanSkillRoots([root]);

  assert.equal(result.errors.length, 0);
  assert.deepEqual(result.skills.map((skill) => skill.name), ['alpha', 'beta']);
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
