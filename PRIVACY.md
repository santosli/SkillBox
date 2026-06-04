# Privacy

SkillBox is designed as a local-first app. The public alpha does not include a
hosted account, telemetry service, or analytics collector.

## Local Data

SkillBox may read:

- configured agent runtime directories such as `~/.codex/skills` and
  `~/.agents/skills`;
- project-local runtime directories such as `.codex/skills` or `.agents/skills`;
- `SKILL.md` files and adjacent skill metadata;
- supported runtime hook config files when you open or modify hook settings.

SkillBox may write:

- managed skill copies and metadata under `~/.skillbox`;
- symlinks from runtime directories to managed skills;
- Git metadata inside `~/.skillbox/user-skills`;
- supported runtime hook config files when you explicitly inject or remove
  hooks.

## Network Access

Network access is limited to workflows that require it, such as installing or
checking remote skills from GitHub or pushing a user-skills Git repository. Those
operations use the remotes and URLs you configure.

## Usage Hooks

Usage hooks are optional. When enabled, they record local skill call metadata so
SkillBox can show usage counts. Hook records are stored locally under the
managed SkillBox data store.

## Removing Data

Normal app uninstall does not remove `~/.skillbox`. See
[docs/uninstall-reset.md](docs/uninstall-reset.md) for reset steps.
