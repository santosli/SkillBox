# Uninstall And Reset

This guide removes the SkillBox app and, optionally, the local data that
SkillBox manages.

## Uninstall The App

If installed from a DMG:

1. Quit SkillBox.
2. Delete `/Applications/SkillBox.app`.

If installed with Homebrew:

```sh
brew uninstall --cask skillbox
```

Normal uninstall does not delete `~/.skillbox`.

## Remove Usage Hooks

Open SkillBox Settings and use the hook controls to remove injected hooks before
deleting the app.

If the app is already removed, inspect supported runtime config files manually:

- `~/.codex/hooks.json`
- `~/.claude/settings.json`

Remove commands that call:

```text
~/.skillbox/bin/skillbox-usage-hook
```

Keep unrelated hook entries intact.

## Remove Runtime Symlinks

Runtime directories are deployment targets. SkillBox deploys managed skills as
symlinks by default.

Inspect runtime roots such as:

```sh
ls -la ~/.codex/skills
ls -la ~/.agents/skills
```

Remove only symlinks that point into `~/.skillbox/user-skills` or
`~/.skillbox/remote-skills`. Do not delete non-symlink directories unless you
created them and have a backup.

## Reset SkillBox Data

This deletes managed skills, the local SQLite index, user-skill Git metadata,
and remote skill copies:

```sh
mv ~/.skillbox ~/.skillbox.backup
```

After confirming the backup is no longer needed:

```sh
rm -rf ~/.skillbox.backup
```

Do not run the delete step until you have verified that no runtime symlink still
depends on the backup.
