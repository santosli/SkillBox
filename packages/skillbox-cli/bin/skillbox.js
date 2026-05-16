#!/usr/bin/env node
import {
  checkRemoteUpdates,
  defaultManagedRoot,
  defaultRuntimeRoots,
  deploySkill,
  ensureManagedLayout,
  importSkill,
  indexSkills,
  installRemoteSkillFromGitHub,
  managedPaths,
  parseGitHubSkillUrl,
  rollbackRemoteSkill,
  scanSkillRoots,
  syncUserSkills,
  VERSION
} from '../../skillbox-core/index.js';

const args = process.argv.slice(2);
const command = args.shift();

try {
  const result = await run(command, args);
  if (result !== undefined) print(result, hasFlag(args, '--json'));
} catch (error) {
  console.error(`skillbox: ${error.message}`);
  process.exitCode = 1;
}

async function run(commandName, commandArgs) {
  switch (commandName) {
    case undefined:
    case 'help':
    case '--help':
    case '-h':
      return helpText();
    case '--version':
    case 'version':
      return VERSION;
    case 'init':
      return ensureManagedLayout(getOption(commandArgs, '--managed-root') || defaultManagedRoot());
    case 'scan': {
      const managedRoot = getOption(commandArgs, '--managed-root') || defaultManagedRoot();
      const roots = positional(commandArgs);
      const scan = scanSkillRoots(roots.length ? roots : defaultRuntimeRoots());
      const indexed = indexSkills(scan.skills, managedRoot);
      return { ...scan, indexed };
    }
    case 'import': {
      const sourceDir = positional(commandArgs)[0];
      if (!sourceDir) throw new Error('Usage: skillbox import <source-dir> --type user|remote');
      return importSkill({
        sourceDir,
        type: getOption(commandArgs, '--type') || 'user',
        managedRoot: getOption(commandArgs, '--managed-root') || defaultManagedRoot()
      });
    }
    case 'install': {
      const url = positional(commandArgs)[0];
      if (!url) throw new Error('Usage: skillbox install <github-url> [--target <path>]');
      return installRemoteSkillFromGitHub({
        url,
        managedRoot: getOption(commandArgs, '--managed-root') || defaultManagedRoot(),
        targetRoot: getOption(commandArgs, '--target')
      });
    }
    case 'deploy': {
      const skillName = positional(commandArgs)[0];
      const targetRoot = getOption(commandArgs, '--target');
      if (!skillName || !targetRoot) {
        throw new Error('Usage: skillbox deploy <skill-name> --target <path>');
      }
      return deploySkill({
        skillName,
        targetRoot,
        managedRoot: getOption(commandArgs, '--managed-root') || defaultManagedRoot()
      });
    }
    case 'check-updates':
      return checkRemoteUpdates({
        managedRoot: getOption(commandArgs, '--managed-root') || defaultManagedRoot(),
        skillName: positional(commandArgs)[0]
      });
    case 'rollback': {
      const skillName = positional(commandArgs)[0];
      const toSha = getOption(commandArgs, '--to');
      if (!skillName || !toSha) throw new Error('Usage: skillbox rollback <skill-name> --to <sha>');
      return rollbackRemoteSkill({
        skillName,
        toSha,
        managedRoot: getOption(commandArgs, '--managed-root') || defaultManagedRoot()
      });
    }
    case 'sync-user-skills':
      return syncUserSkills({
        managedRoot: getOption(commandArgs, '--managed-root') || defaultManagedRoot(),
        remote: getOption(commandArgs, '--remote'),
        commitMessage: getOption(commandArgs, '--message'),
        push: hasFlag(commandArgs, '--push')
      });
    case 'parse-github-url': {
      const url = positional(commandArgs)[0];
      if (!url) throw new Error('Usage: skillbox parse-github-url <github-url>');
      return parseGitHubSkillUrl(url);
    }
    case 'paths':
      return managedPaths(getOption(commandArgs, '--managed-root') || defaultManagedRoot());
    default:
      throw new Error(`Unknown command: ${commandName}`);
  }
}

function print(value, json) {
  if (typeof value === 'string') {
    console.log(value);
    return;
  }
  if (json) {
    console.log(JSON.stringify(value, null, 2));
    return;
  }
  console.log(pretty(value));
}

function pretty(value) {
  if (Array.isArray(value)) return value.map((item) => pretty(item)).join('\n');
  if (!value || typeof value !== 'object') return String(value);
  return Object.entries(value)
    .map(([key, entry]) => `${key}: ${typeof entry === 'object' ? JSON.stringify(entry) : entry}`)
    .join('\n');
}

function getOption(values, name) {
  const index = values.indexOf(name);
  if (index === -1) return undefined;
  return values[index + 1];
}

function hasFlag(values, name) {
  return values.includes(name);
}

function positional(values) {
  const result = [];
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (value.startsWith('--')) {
      if (index + 1 < values.length && !values[index + 1].startsWith('--')) index += 1;
      continue;
    }
    result.push(value);
  }
  return result;
}

function helpText() {
  return `SkillBox ${VERSION}

Commands:
  skillbox init [--managed-root <path>]
  skillbox scan [root ...] [--managed-root <path>] [--json]
  skillbox import <source-dir> --type user|remote [--managed-root <path>]
  skillbox install <github-url> [--target <path>] [--managed-root <path>]
  skillbox deploy <skill-name> --target <path> [--managed-root <path>]
  skillbox check-updates [skill-name] [--managed-root <path>]
  skillbox rollback <skill-name> --to <sha> [--managed-root <path>]
  skillbox sync-user-skills [--remote <git-url>] [--message <msg>] [--push]
  skillbox parse-github-url <github-url>
  skillbox paths [--managed-root <path>]
`;
}
