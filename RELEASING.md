# Releasing — maintainer guide

How to publish a new version of Prayer Times. The whole process is ~2 minutes
of work; GitHub Actions does the building for all platforms.

> **TL;DR** — bump the version in 3 files → commit → push → check CI is green
> → tag → push the tag. Done.

## One-time setup (already done — do not repeat)

| What | Where | Notes |
| ---- | ----- | ----- |
| Updater signing keypair | `~/.tauri/prayer-times.key` (+ `.pub`) | Generated with `pnpm tauri signer generate`. **Backed up somewhere safe** — see warning below. |
| Public key | `src-tauri/tauri.conf.json` → `plugins.updater.pubkey` | Baked into every build; verifies update signatures. |
| `TAURI_SIGNING_PRIVATE_KEY` secret | GitHub → Settings → Secrets and variables → Actions | Lets the Release workflow sign update files. |
| "release tags" ruleset | GitHub → Settings → Rules → Rulesets | Only `iamraihan` may create, move or delete `v*` tags. Source: `.github/rulesets/release-tags.json`; apply with `gh api -X POST repos/iamraihan/prayer-times/rulesets --input .github/rulesets/release-tags.json`. |

### Who can publish a release?

Only someone who can push a `v*` tag to **this** repo. Forks can push tags
to their own copy, but that runs the workflow in the fork, without this
repo's secrets, and publishes nothing here. Inside this repo, the "release
tags" ruleset restricts `v*` tags to the maintainer, and the Release
workflow's `verify` job refuses to build when the tag is not on `main` or
doesn't match the version files.

⚠️ **Never lose the private key.** Every app already installed only trusts
updates signed with it. If it's lost, all existing installs lose auto-updates
forever and must reinstall manually.

## Every release, step by step

Say the next version is `0.3.0`:

1. **Bump the version to `0.3.0` in all three files** (keep them identical):

   | File | Field |
   | ---- | ----- |
   | `src-tauri/tauri.conf.json` | `"version"` ← the one the updater reads |
   | `src-tauri/Cargo.toml` | `version` |
   | `package.json` | `"version"` |

   Quick check: `grep '"version"' package.json src-tauri/tauri.conf.json && grep '^version' src-tauri/Cargo.toml`

   (Optional but tidy: `cargo check --manifest-path src-tauri/Cargo.toml`
   once so `Cargo.lock` records the new version too.)

2. **Commit and push:**

   ```bash
   git add -A
   git commit -m "release v0.3.0"
   git push
   ```

3. **Wait for CI to go green** (Actions tab → "CI"). Never tag a red build.

4. **Tag and push the tag — the tag number must match the files:**

   ```bash
   git tag v0.3.0
   git push origin v0.3.0
   ```

5. **Watch the Release workflow** (Actions tab → "Release"). Four parallel
   builds run: Windows, Ubuntu, macOS (Apple Silicon + Intel). Takes ~15–20 min.

6. **Check the result** on the Releases page: installers for all platforms,
   plus the signed update files and `latest.json` that installed apps check.

Users' apps detect the update on their next launch: a banner appears, the
update downloads inside the app (Windows/Linux AppImage) and the app restarts.
On macOS the app links to the release page (silent updates there need a paid
Apple Developer ID certificate).

## Choosing the version number

| Bump | Use when | Example |
| ---- | -------- | ------- |
| Patch `0.3.0 → 0.3.1` | bug fix, small tweak | fixed alarm not firing |
| Minor `0.3.0 → 0.4.0` | new feature, settings option | added new notification type |
| Major `0.x → 1.0` | big milestone / breaking change | first "stable" release |

## Troubleshooting

| Symptom | Cause | Fix |
| ------- | ----- | --- |
| `git push` to main rejected (`GH006: Protected branch`) | Your user is missing from the ruleset bypass list | Settings → Rules → Rules → edit ruleset → Bypass list → add `iamraihan` |
| Release workflow fails at signing (`missing private key` / env var error) | `TAURI_SIGNING_PRIVATE_KEY` secret missing or wrong content | Re-add the secret; content must be the **entire** `~/.tauri/prayer-times.key` file |
| `verify` job fails: "does not match the version files" | Tag number ≠ version in one of the three files | Delete the tag (below), fix files, re-tag |
| `verify` job fails: "not on main" | Tagged a commit that isn't on `main` yet (e.g. a local or feature-branch commit) | Delete the tag, push/merge the commit to `main`, re-tag |
| `git push origin vX.Y.Z` rejected (`GH013: Repository rule violations`) | Your user is missing from the "release tags" ruleset bypass list | Settings → Rules → Rulesets → "release tags" → Bypass list → add `iamraihan` |
| Tagged by mistake / want to redo | — | `git tag -d v0.3.0` (local), `git push origin :refs/tags/v0.3.0` (remote), delete the release on GitHub if it was created, then fix and re-tag |
| Linux build fails: `Package alsa was not found` or another missing system library | A system dev package missing from the workflow's `apt-get install` list | Add the package in **both** `.github/workflows/release.yml` and `ci.yml`, commit, then delete and re-push the tag (re-runs use the old workflow file) |
| Users don't see the update | They installed a version from before the updater existed (e.g. 0.1.0) | Those users must download v0.2.0+ manually once — from then on updates are automatic |
| CI red | Code problem | Fix and push **before** tagging; never release on red |

## Notes

- Tags point at a commit — the version bump commit is what gets tagged, so
  always **tag last**, after pushing the bump.
- The Release workflow (`.github/workflows/release.yml`) only triggers on tags
  matching `v*`. CI (`.github/workflows/ci.yml`) runs on every push to main
  and every PR.
- `.cargo/config.toml` is machine-local (gitignored) — the repo must never
  carry build-output redirects like the old `E:/cargo-target` path; it breaks
  the CI/build runners.
