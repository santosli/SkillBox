# Public Alpha Release

SkillBox public alpha releases target macOS 14+ and publish a signed,
notarized, universal DMG through GitHub Releases.

## Release Identity

- Organization: `skillbox-dev`
- Main repository: `skillbox-dev/skill-box`
- Homebrew tap: `skillbox-dev/homebrew-tap`
- Bundle identifier: `io.github.skillbox-dev.skillbox`
- First tag: `v0.1.0-alpha.1`
- First DMG asset: `SkillBox_0.1.0-alpha.1_universal.dmg`

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
3. Tag the release:

   ```sh
   git tag v0.1.0-alpha.1
   git push origin v0.1.0-alpha.1
   ```

4. Wait for `.github/workflows/release.yml` to publish the prerelease.
5. Download and smoke-test the DMG.
6. Copy `packaging/homebrew/Casks/skillbox.rb` into
   `skillbox-dev/homebrew-tap/Casks/skillbox.rb`.
7. Replace the placeholder SHA with the value from the release `SHA256SUMS`.
8. Run:

   ```sh
   brew audit --cask skillbox-dev/tap/skillbox
   brew install --cask skillbox-dev/tap/skillbox
   brew uninstall --cask skillbox-dev/tap/skillbox
   ```

9. Commit and push the tap update.

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
