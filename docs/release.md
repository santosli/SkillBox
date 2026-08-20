# Release

SkillBox releases target macOS 14+ and publish a signed, notarized, universal
DMG plus Tauri updater artifacts through GitHub Releases.

The DMG itself is a release artifact and must be submitted to Apple notarization,
reach `Accepted`, be stapled, and pass both `xcrun stapler validate` and
`spctl --assess --type open -vv --context context:primary-signature` before it
can be published. The primary-signature context is required for a disk-image
Gatekeeper assessment. App-level signing and mounted-app checks do not replace
these DMG-level checks.

## Release Identity

- Publishing account: `santosli`
- Main repository: `santosli/SkillBox`
- Homebrew tap: `santosli/homebrew-tap`
- Bundle identifier: `io.github.santosli.skillbox`
- Current tag: `v0.9.1`
- Current DMG asset: `SkillBox_0.9.1_universal.dmg`
- Current updater asset: `SkillBox_0.9.1_universal.app.tar.gz`
- Current updater signature: `SkillBox_0.9.1_universal.app.tar.gz.sig`
- Current updater manifest: `latest.json`
- Current checksum asset: `SkillBox_0.9.1_universal.dmg.sha256`

## GitHub Actions Secrets

Configure these secrets before pushing a release tag:

- `APPLE_CERTIFICATE`
- `APPLE_CERTIFICATE_PASSWORD`
- `APPLE_SIGNING_IDENTITY`
- `APPLE_ID`
- `APPLE_PASSWORD`
- `APPLE_TEAM_ID`
- `KEYCHAIN_PASSWORD`
- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (optional when the updater key has no password)

`APPLE_CERTIFICATE` should contain a base64-encoded `.p12` Developer ID
Application certificate. `APPLE_PASSWORD` should be an app-specific password.
`TAURI_SIGNING_PRIVATE_KEY` should contain the Tauri updater private key
content, not the public key committed in `tauri.conf.json`. Keep an offline
backup; losing this key prevents existing installs from accepting future
updates.

## Release Command

Run releases through `scripts/release.js` instead of replaying manual GitHub,
tag, checksum, and tap steps.

1. Start on a clean, up-to-date `main` branch.
2. Write the main changes as bullets in a temporary notes file:

   ```sh
   cat > /tmp/skillbox-release-notes.md
   ```

3. Run the full release:

   ```sh
   npm run release -- 0.3.0 --notes-file /tmp/skillbox-release-notes.md --yes
   ```

The command:

- updates package, Rust crate, Tauri, README, SECURITY, roadmap, issue template,
  release doc, lockfile, and changelog versions;
- runs the local release checks;
- commits and pushes the release-prep change to `main`;
- runs the `Release` workflow once through `workflow_dispatch` as a no-publish
  dry run;
- creates and pushes the `v<version>` tag;
- waits for the tag-triggered Release workflow to build, submit the DMG to
  notarization, wait for `Accepted`, staple and validate the DMG, mount and
  verify the app, publish, upload updater artifacts, and upload checksums;
- reads the published DMG checksum from GitHub Releases;
- verifies the published release includes the DMG, updater archive, updater
  signature, and `latest.json`;
- updates and pushes `packaging/homebrew/Casks/skillbox.rb`;
- updates and pushes `santosli/homebrew-tap`.

If the process is interrupted after the tag-triggered workflow has published
the release, rerun `npm run release:publish -- <version> --yes`. The runner
verifies that the local and remote tag (lightweight or annotated) agree, the
published release exists, and the tag is an ancestor of the current main before
resuming DMG digest, cask, and tap updates.

Useful variants:

```sh
npm run release:prepare -- 0.3.0 --notes-file /tmp/skillbox-release-notes.md
npm run release:publish -- 0.3.0 --yes
npm run release -- 0.3.0 --notes-file /tmp/skillbox-release-notes.md --yes --skip-tap
```

Use `--tap-dir <path>` to reuse an existing local checkout of
`santosli/homebrew-tap`.

The GitHub Release body is generated from the matching `CHANGELOG.md` section.
The release workflow fails if that section is missing.
Release assets are uploaded from versioned filenames under `release-assets/`;
the updater URLs in `latest.json` must match those asset filenames, not only
GitHub release labels.

## Smoke Test

- Install the DMG on a fresh macOS user profile.
- Verify the downloaded DMG itself before opening it:

  ```sh
  xcrun stapler validate SkillBox_<version>_universal.dmg
  spctl --assess --type open -vv --context context:primary-signature SkillBox_<version>_universal.dmg
  ```

- Verify Gatekeeper accepts the app:

  ```sh
  spctl -a -vv /Applications/SkillBox.app
  ```

- Launch the app.
- Verify the background updater check does not download automatically. When a
  test update exists, confirm the sidebar Update action survives an app restart
  within the 24-hour cache window.
- Scan workspaces.
- Import one test skill.
- Deploy and undeploy one symlink.
- Inject and remove usage hooks.
- Verify the Homebrew cask installs, upgrades, uninstalls, and does not delete
  `~/.skillbox`.
- After publishing a new release, launch the previous DMG build and verify it
  can find the new version, revalidate it from one Update click, install it,
  and restart. Keep the previous build open across the due window to verify
  long-running daily checks as part of release qualification.
