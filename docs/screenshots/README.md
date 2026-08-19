# Screenshot Maintenance

Refresh public screenshots after a visible desktop workflow, navigation,
terminology, or layout change affects the README or homepage story.

## Capture

- Run the deterministic browser fixture with `?public-preview=1`; never capture
  real managed paths, usernames, prompts, chat content, private skill names, or
  personal usage counts.
- Use the current version-neutral filenames in this directory. Do not add
  release numbers or dates to canonical screenshot names.
- Capture the relevant desktop state and inspect a narrow viewport for
  clipping, overlap, horizontal overflow, and unreadable controls.
- Treat browser preview as visual fixture evidence only. It does not replace
  packaged Tauri verification for native filesystem, dialog, updater, or
  operating-system behavior.

## Update References

- Check every image reference in `README.md` and `README.zh-CN.md`.
- Keep canonical README and homepage product screenshots in this directory;
  `site/build.mjs` copies them into the built site with version-neutral names.
- Keep promo assets under `docs/promo/skillbox-intro/assets/` internally aligned
  with the committed promo composition. Refresh them only as part of an
  intentional promo update; a README/homepage screenshot refresh does not by
  itself require changing the video source.
- Refresh the existing promo composition only when the changed UI or message is
  visible in the public video; do not create a second unrelated promo source.
- Remove obsolete duplicate assets only after confirming no README, site,
  promo, or source file references them.

## Verify

```sh
node site/build.mjs
node site/validate.mjs
git diff --check
```

Also verify that every referenced README/site media path exists and review the
desktop and narrow captures before committing binary replacements.
