# Good First Issues

Good first issues help new contributors make a useful change without needing to
understand every SkillBox safety boundary.

## For Contributors

Start with issues labeled
[`good first issue`](https://github.com/santosli/SkillBox/labels/good%20first%20issue)
or [`help wanted`](https://github.com/santosli/SkillBox/labels/help%20wanted).

Before starting:

- read the issue acceptance criteria;
- check whether the issue names the likely files to touch;
- ask on the issue if the expected behavior is ambiguous;
- keep the first patch small.

Useful local checks:

```sh
npm test
cargo test --offline
cargo fmt --check
git diff --check
```

If a change touches Rust code, also run:

```sh
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

## Good Starter Tasks

Good starter tasks usually have:

- one clear user-visible improvement or test gap;
- a small set of likely files;
- no destructive filesystem behavior;
- no signing, notarization, or release-secret dependency;
- an obvious verification command.

Examples:

- fix stale release text in docs;
- add tests for a pure helper function;
- improve an empty state or button label;
- update screenshots when the UI already works;
- add a small issue template or contributor checklist.

## Not Good First Issues

These need a maintainer or deeper project context:

- import, deploy, undeploy, or rollback behavior that can affect user files;
- hook injection or config rewriting;
- GitHub download or remote update trust boundaries;
- SQLite schema changes or migrations;
- release signing, notarization, and Homebrew publication;
- broad UI rewrites.

## Maintainer Checklist

When creating a starter issue:

- use the `starter:` title prefix;
- apply `good first issue` and `help wanted`;
- describe the user-facing problem;
- list likely files to inspect;
- include acceptance criteria;
- include verification commands;
- call out anything that must not be changed.
