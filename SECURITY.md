# Security Policy

SkillBox is early-stage software that reads and writes local skill directories.
Please report security issues privately before opening public issues.

## Supported Versions

Only the latest public release is supported for security fixes.

| Version | Supported |
| --- | --- |
| `0.2.x` | Yes |
| `0.1.x` | No |
| `0.1.0-alpha.x` | No |
| older builds | No |

## Reporting A Vulnerability

Use GitHub private vulnerability reporting for the repository when available.
If private reporting is not yet enabled, open a minimal public issue that says a
private security report is needed, without exploit details or sensitive paths.

Include:

- affected version or commit;
- operating system version;
- exact workflow involved;
- expected and actual behavior;
- whether local files, symlinks, hooks, Git remotes, or downloaded archives are
  involved.

## Security Expectations

SkillBox should:

- treat GitHub URLs, archives, runtime folders, and existing skills as untrusted;
- avoid executing user-provided shell strings;
- avoid silently overwriting non-symlink runtime targets;
- preserve user-created skill content unless destructive action is confirmed;
- keep `~/.skillbox` as user-controlled local data.
