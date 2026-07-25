# To create a release

Releases are built by [`dist`](https://opensource.axo.dev/cargo-dist/) in
[`.github/workflows/release.yml`](.github/workflows/release.yml), which is
generated from `dist-workspace.toml` — edit the config and re-run `dist init`
rather than hand-editing the workflow.

Pushing a version tag is what triggers a release.

1. Bump `version` in `Cargo.toml` and run `cargo check` to update the lock file.
1. Bump the two pinned installer URLs in `README.md` — they point at
   `releases/download/v<VERSION>/` so the documented command always matches a
   release that exists.
1. Rename the `## [Unreleased]` header in `CHANGELOG.md` to the new version and
   date. Use `### Added/Fixed/Changed` subheaders as appropriate — the release
   body on GitHub is generated from this section.
1. Commit and push those changes.
1. Tag it, e.g. `git tag -a v0.27.0`, and put the changelog entry in the tag body.
1. Push the tag: `git push --tags`.
1. Watch the run on the [Actions page](https://github.com/sandeshgrangdan/spotify-tui/actions).

On success the workflow creates the GitHub Release and attaches, for all seven
targets, the binary archives, the `spotify-tui-installer.sh` /
`spotify-tui-installer.ps1` installers, the npm package tarball, and a
`sha256.sum`.

To check what a release will contain without tagging anything, run `dist plan`.
`dist build --artifacts=global` renders the installers locally into
`target/distrib/`.

## Notes

- **crates.io is not a release target.** `publish = false` is set in
  `Cargo.toml`; the `spotify-tui` name there belongs to upstream. The
  accompanying `[package.metadata.dist] dist = true` is what keeps `publish =
  false` from hiding the package from `dist`.
- **npm publishing needs an `NPM_TOKEN` repo secret.** `publish-jobs = ["npm"]`
  adds the `publish-npm` job to `release.yml`, which runs
  `npm publish --access public` with `NODE_AUTH_TOKEN` taken from
  `secrets.NPM_TOKEN`. Generate that on npm as an **Automation** token, so 2FA
  doesn't block it, and add it under Settings → Secrets and variables → Actions.
  The job is skipped for prerelease tags unless `publish-prereleases` is set.
- There is currently no test/lint CI. `release.yml` runs `dist plan` on pull
  requests, but nothing runs `cargo test`, `fmt` or `clippy`.
