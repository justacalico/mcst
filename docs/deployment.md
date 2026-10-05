# Deployment & CI notes

## Repositories

- Source of truth: <https://gitlab.com/HttpAnimations/mcst>
- Mirror + release builds: <https://github.com/justacalico/mcst>
- GitLab shared runner: `55109445` (`linux-truenas` tag), already attached.

## Pipeline flow

1. Push to `main` on GitLab → `github-sync` mirrors `main` (and tags) to GitHub
   and waits for the GitHub Actions `build.yml` run to succeed.
2. `auto-release` runs cocogitto: if commits warrant a bump it creates a
   version commit + `v*` tag → tag push starts the tag pipeline.
3. The tag pipeline mirrors the tag → GitHub `build.yml` builds binaries for
   Linux x86_64/arm64, Windows x86_64, macOS arm64 and creates a GitHub release.
4. `docker-image` builds + pushes `registry.gitlab.com/httpanimations/mcst:<tag>`.
5. `gitlab-sync` (GitHub side) triggers the GitLab `github-release-sync` job,
   which mirrors the GitHub release assets into a GitLab release.
6. `github-dispatch` (web pipeline) and `github-mr-build` (MR pipelines)
   trigger GitHub builds manually / per merge request.

## Required GitLab CI/CD variables

Set at *Settings → CI/CD → Variables* (masked + protected where noted):

| Variable | Purpose |
|---|---|
| `GITHUB_SSH_PRIVATE_KEY` | SSH deploy key with write access to `justacalico/mcst` — used to push `main` and tags. |
| `GITLAB_TOKEN` | PAT with `api` scope — used by `auto-release` to push the version bump commit + tag. |
| `GITLAB_RELEASE_SSH_KEY` | SSH key for the GitLab repo — lets `github-release-sync` move the tag to the released commit. |

`GH_TOKEN` / `GITHUB_TOKEN` for `gh` inside GitLab jobs come from the runner
environment or a `GITHUB_TOKEN` variable if your runners don't provide one.

## Required GitHub secrets

Set at *github.com/justacalico/mcst → Settings → Secrets → Actions*:

| Secret | Purpose |
|---|---|
| `GITLAB_TRIGGER_TOKEN` | GitLab pipeline trigger token for `gitlab-sync` (create at GitLab → Settings → CI/CD → Pipeline triggers). |
| `GITLAB_PROJECT_ID` | Numeric project id (`87261408`). |

## Notes

- No `.env` files anywhere — the app reads `-p/--port`, `--host`,
  `-d/--data` only.
- `MIN_COVERAGE` (default 85) gates frontend line coverage in
  `scripts/check-coverage.sh`.
- Releases are created by cocogitto (`cog.toml`, hook profile `ci`); the
  `commit-msg` hook keeps non-conventional subjects compatible.
- Docker builds on tag pipelines only; `docker compose up -d` works locally
  from a plain checkout.
