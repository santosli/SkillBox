# Public Alpha Release

SkillBox public alpha releases target macOS 14+ and publish a signed,
notarized, universal DMG through GitHub Releases.

## Release Identity

- Publishing account: `santosli`
- Main repository: `santosli/skill-box`
- Homebrew tap: `santosli/homebrew-tap`
- Bundle identifier: `io.github.santosli.skillbox`
- Current tag: `v0.1.0-alpha.3`
- Current DMG asset: `SkillBox_0.1.0-alpha.3_universal.dmg`
- Current checksum asset: `SkillBox_0.1.0-alpha.3_universal.dmg.sha256`

## GitHub Actions Secrets

Configure these secrets before pushing a release tag:

- `APPLE_CERTIFICATE`
- `APPLE_CERTIFICATE_PASSWORD`
- `APPLE_SIGNING_IDENTITY`
- `APPLE_ID`
- `APPLE_PASSWORD`
- `APPLE_TEAM_ID`
- `KEYCHAIN_PASSWORD`

`APPLE_CERTIFICATE` should contain a base64-encoded `.p12` Developer ID
Application certificate. `APPLE_PASSWORD` should be an app-specific password.

## Release Steps

1. Confirm `main` is clean and CI is passing.
2. Confirm README install and uninstall instructions match the current release.
3. Run the `Release` workflow manually from `main` before tagging. The
   workflow dispatch path builds, notarizes, mounts, and verifies the DMG
   without creating a GitHub Release.
4. Tag the release:

   ```sh
   git tag v0.1.0-alpha.3
   git push origin v0.1.0-alpha.3
   ```

5. Wait for `.github/workflows/release.yml` to build, notarize, mount, and
   verify the DMG before publishing the prerelease. The workflow must pass
   `codesign --verify`, `spctl`, app version, and bundle identifier checks
   against the mounted DMG.
6. Download and smoke-test the published DMG.
7. Copy `packaging/homebrew/Casks/skillbox.rb` into
   `santosli/homebrew-tap/Casks/skillbox.rb`.
8. Replace the placeholder SHA with the value from the release checksum asset
   or `SHA256SUMS`.
9. Run:

   ```sh
   brew audit --cask santosli/tap/skillbox
   brew install --cask santosli/tap/skillbox
   brew uninstall --cask santosli/tap/skillbox
   ```

10. Commit and push the tap update.

## Smoke Test

- Install the DMG on a fresh macOS user profile.
- Verify Gatekeeper accepts the app:

  ```sh
  spctl -a -vv /Applications/SkillBox.app
  ```

- Launch the app.
- Scan workspaces.
- Import one test skill.
- Deploy and undeploy one symlink.
- Inject and remove usage hooks.
- Verify the Homebrew cask installs, upgrades, uninstalls, and does not delete
  `~/.skillbox`.
