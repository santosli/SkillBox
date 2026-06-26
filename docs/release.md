# Release

SkillBox releases target macOS 14+ and publish a signed, notarized, universal
DMG plus Tauri updater artifacts through GitHub Releases.

## Release Identity

- Publishing account: `santosli`
- Main repository: `santosli/SkillBox`
- Homebrew tap: `santosli/homebrew-tap`
- Bundle identifier: `io.github.santosli.skillbox`
- Current tag: `v0.3.5`
- Current DMG asset: `SkillBox_0.3.5_universal.dmg`
- Current updater asset: `SkillBox_0.3.5_universal.app.tar.gz`
- Current updater signature: `SkillBox_0.3.5_universal.app.tar.gz.sig`
- Current updater manifest: `latest.json`
- Current checksum asset: `SkillBox_0.3.5_universal.dmg.sha256`

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
- waits for the tag-triggered Release workflow to build, notarize, mount,
  verify, publish, upload updater artifacts, and upload checksums;
- reads the published DMG checksum from GitHub Releases;
- verifies the published release includes the DMG, updater archive, updater
  signature, and `latest.json`;
- updates and pushes `packaging/homebrew/Casks/skillbox.rb`;
- updates and pushes `santosli/homebrew-tap`.

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
- Verify Gatekeeper accepts the app:

  ```sh
  spctl -a -vv /Applications/SkillBox.app
  ```

- Launch the app.
- In Settings -> App updates, verify the updater status renders without trying
  to install anything automatically.
- Scan workspaces.
- Import one test skill.
- Deploy and undeploy one symlink.
- Inject and remove usage hooks.
- Verify the Homebrew cask installs, upgrades, uninstalls, and does not delete
  `~/.skillbox`.
- After publishing a new release, launch the previous DMG build and verify it
  can find the new version, install it after confirmation, and restart.
