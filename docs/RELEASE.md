# Release process

Releases are built and published automatically by `.github/workflows/release.yml`.

## Artifacts produced

| Platform | Target | Artifacts |
|---|---|---|
| Windows | `x86_64-pc-windows-msvc` | `discord-phoenix-<version>-windows-x86_64.zip`, `discord-phoenix-<version>-windows-x86_64.exe` |
| Linux | `x86_64-unknown-linux-gnu` | `discord-phoenix-<version>-x86_64.AppImage`, `discord-phoenix_<deb_version>_amd64.deb`, `discord-phoenix-<version>-linux-x86_64.tar.gz` |
| macOS | Universal (`aarch64-apple-darwin` + `x86_64-apple-darwin` via `lipo`) | `discord-phoenix-<version>-macos-universal.dmg`, `discord-phoenix-<version>-macos-universal.app.zip` |
| Checksums | All | `SHA256SUMS.txt` |

## Triggering a release

### Option 1: Git tag push (recommended)

1. Update `version` under `[workspace.package]` in `Cargo.toml` and commit to `main`.
2. Tag and push:
   ```bash
   git tag v0.1.0-beta.1
   git push origin v0.1.0-beta.1
   ```
   Any tag containing `-` (e.g. `-alpha`, `-beta`, `-rc`) is automatically marked as a GitHub prerelease.

### Option 2: Manual dispatch (`workflow_dispatch`)

1. Open **Actions → Release → Run workflow** on GitHub.
2. Optionally set `tag` (defaults to `v<workspace.package.version>` from `Cargo.toml`) and `prerelease` (`auto`, `true`, or `false`).
3. Run the workflow to build all targets and create/update the GitHub Release.
