# Development

How this repo builds, releases and publishes `typst-codex`.

## Layout

| Path | Function |
|-|-|
| [`generator/`](generator/src/main.rs) | Rust binary that walks `codex::SYM` and `codex::EMOJI` and writes the package. Also plans each release. |
| [`typst-codex/`](typst-codex/) | The Espanso package. `package.yml` is generated – never edit it by hand. |
| [`ci.yml`](.github/workflows/ci.yml) | Runs the generator's tests on PRs and `main`. |
| [`automerge.yml`](.github/workflows/automerge.yml) | Auto-merges Dependabot PRs once CI passes. |
| [`release.yml`](.github/workflows/release.yml) | Plans a release weekly, waits for approval, then releases and opens a Hub PR. |
| [`dependabot.yml`](.github/dependabot.yml) | Weekly codex and GitHub Actions updates, on Mondays. |

## Local Development

Requires Rust 1.88 or newer.

```sh
cd generator
cargo run     # regenerate typst-codex/
cargo test    # run the tests
```

`cargo run` rewrites `package.yml` and updates the codex version mentioned in `_manifest.yml` and the package README.

To preview a release against the last tag:
```sh
git worktree add ../last $(git tag --list 'v*' --sort=-v:refname | head -n 1)
cargo run -- release auto ../last/typst-codex
git worktree remove ../last
```

This prints the summary you'd see when approving, and bumps the version in `_manifest.yml` – discard that change afterwards.

## Release Flow

1. **Monday** – Dependabot opens PRs for codex and GitHub Actions updates. Each one auto-merges once CI passes.
2. **Tuesday** – **Release** regenerates the package and diffs it against the last `v*` tag.
   - Unchanged? The run ends there, with no request for you.
   - Triggers removed without a major bump? The run fails and lists them.
3. **Approval** – if the package changed, the run's summary lists the new version, the bump, and every added, removed and changed symbol. The `publish` job waits for you.
   - **Approve** to release.
   - **Reject** to skip it. Next week's run proposes it again, with any newer changes included.
4. **Publish** – after approval, the workflow:
   - force-pushes the `release` branch (`main` plus one release commit) and tags `vX.Y.Z`
   - creates a GitHub release with the summary as its notes
   - syncs your Hub fork with espanso/hub, pushes `packages/typst-codex/X.Y.Z/` to a branch, and opens a PR to espanso/hub

Run **Release** from the Actions tab to skip the wait, or to choose the bump yourself.

### Versioning

The latest `v*` tag is the source of truth. The version in `main`'s `_manifest.yml` is only the starting point, and the release commit on the `release` branch carries the real one.

Releases only happen when the regenerated `package.yml` differs from the last release:

| Change | Bump |
|-|-|
| codex version changed | minor |
| Any other output change (labels, new fields) | patch |
| Removed triggers or a trigger syntax change | major – run **Release** manually with `bump: major` |
| Refactors, CI or README changes | none |

## Setup

One-time setup for a fresh copy of this repo.

### 1. Repo settings

1. **General** – enable **Allow auto-merge**.
2. **Branches** – add a rule for `main` that requires the `ci` status check. Auto-merge relies on it to wait for CI. Leave `release` unprotected, because the workflow force-pushes it.
3. **Environments** – create `release` and add yourself under **Required reviewers**. This is the approval step.

### 2. First release

v0.1.0 is done by hand. Tag it and create the `release` branch from `main`:
```sh
git tag v0.1.0
git push origin v0.1.0 main:release
```

Then submit to the Hub by hand:

1. Fork [espanso/hub](https://github.com/espanso/hub).
2. Copy `typst-codex/` to `packages/typst-codex/0.1.0/` in your fork.
3. Open a PR to espanso/hub.

**Release** fails until the `v0.1.0` tag exists, since every release is diffed against the last tag.

### 3. Hub automation

Once the package is on the Hub, let releases open the Hub PRs for you. Under **Settings → Secrets and variables → Actions**:

| Name | Type | Value |
|-|-|-|
| `HUB_FORK` | variable | Your fork, e.g. `munr0/hub` |
| `HUB_AUTOMATION` | variable | `true`. Leave unset to release here only. |
| `HUB_TOKEN` | `release` environment secret | Fine-grained PAT for your fork only, with **Contents** and **Pull requests** read and write |

Storing `HUB_TOKEN` in the `release` environment means it's only available after you approve.

## Notes

- GitHub disables scheduled workflows after 60 days with no repo activity. Dependabot's merges should keep it active. If the Tuesday runs stop, re-enable **Release** in the Actions tab.
- Dependabot's merges are made with `GITHUB_TOKEN`, so they don't trigger other workflows. That's why **Release** runs on a schedule rather than on push.
