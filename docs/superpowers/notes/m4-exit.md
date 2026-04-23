# M4 Exit — CI Adoption (Option A)

Date: 2026-04-23  
Branch: chore/upstream-sync-v0.7.3

## Decision: Option A — minimal adoption, no overlay workflow

No new GitHub Actions workflow was added. Rationale:
- Dev iteration frequency does not yet justify autonomous CI builds.
- Aura's existing `infra/ecr/build-and-push.sh` remains the image-build mechanism.
- Upstream v0.7.3 CI workflows run on the fork as-is; some produce noisy failures (see below) but none block anything downstream.

## Upstream workflows inherited from v0.7.3

Total: 15 workflow files

1. `ci.yml` — main CI (build + test)
2. `cross-platform-build-manual.yml` — manual multi-arch builds
3. `discord-release.yml` — Discord webhook for releases
4. `master-branch-flow.md` — documentation (not executable)
5. `pr-path-labeler.yml` — auto-label PRs based on changed paths
6. `pre-release-validate.yml` — pre-release validation and checks
7. `pub-aur.yml` — publish to AUR (Arch User Repository)
8. `pub-homebrew-core.yml` — publish to Homebrew Core
9. `pub-scoop.yml` — publish to Scoop (Windows package manager)
10. `release-beta-on-push.yml` — automatic beta release on push
11. `release-stable-manual.yml` — manual stable release
12. `sync-marketplace-templates.yml` — sync marketplace templates
13. `tweet-release.yml` — tweet release announcements
14. `version-sync.yml` — sync version information
15. `README.md` — documentation (not executable)

## Known-noisy workflows on our fork

Workflows that will likely fail on our fork due to upstream-only secret / org references. These are red-X status marks only — they do not block merges or deployments:

1. **release-beta-on-push.yml**: Has guards `if: github.repository == 'zeroclaw-labs/zeroclaw'` at the job level, so it will safely skip on our fork.

2. **release-stable-manual.yml**: References `${{ secrets.WEBSITE_REPO_PAT }}` and `zeroclaw-labs/zeroclaw-website` dispatch; will fail at secret-retrieval time if triggered, but does not self-gate.

3. **pre-release-validate.yml**: Requires secrets `RELEASE_TOKEN`, `HOMEBREW_CORE_BOT_TOKEN`, `HOMEBREW_UPSTREAM_PR_TOKEN`, `AUR_SSH_KEY`, `WEBSITE_REPO_PAT`. Will fail at job start if any upstream release workflow is manually triggered.

4. **pub-aur.yml**: Requires `AUR_SSH_KEY` secret; will fail if triggered manually on fork.

5. **pub-homebrew-core.yml**: Requires `HOMEBREW_UPSTREAM_PR_TOKEN` and `HOMEBREW_CORE_BOT_TOKEN` secrets; will fail if triggered.

6. **pub-scoop.yml**: References `zeroclaw-labs/scoop-zeroclaw` repo variable; requires `SCOOP_BUCKET_REPO` variable and likely secrets.

7. **sync-marketplace-templates.yml**: References `zeroclaw-labs/coolify` and `zeroclaw-labs/easypanel` repos; will fail at secret-retrieval time.

8. **tweet-release.yml**: Requires Twitter API secrets (`TWITTER_CONSUMER_API_KEY`, `TWITTER_CONSUMER_API_SECRET_KEY`, `TWITTER_ACCESS_TOKEN`, `TWITTER_ACCESS_TOKEN_SECRET`); will fail if triggered.

**Safe workflows** (will run without failure on fork):
- `ci.yml` — standard build + test, no fork-specific refs
- `cross-platform-build-manual.yml` — manual invocation only, no secrets
- `discord-release.yml` — requires Discord webhook secret, but self-gates or has fallback
- `pr-path-labeler.yml` — pure static path labeling
- `version-sync.yml` — version file sync only

## Dev image build flow (unchanged from pre-sync)

To build a new dev image from the sync branch:

```bash
cd /Users/danielhuynh/Documents/sain/aura
ZEROCLAW_SRC=/Users/danielhuynh/Documents/sain/zeroclaw/.worktrees/sync-v073 \
PROMOTE_LATEST=true \
./infra/ecr/build-and-push.sh
```

Optionally retag `latest` → `dev-latest` per the runbook in `CLAUDE.md` §2.1.

**Script behavior:**
- Takes `ZEROCLAW_SRC` env var (required) pointing at zeroclaw source directory
- Builds `linux/arm64` image via Docker buildx
- Uses `--build-context zeroclaw-src={ZEROCLAW_SRC}` to inject the worktree into the build
- Pushes to ECR; if `PROMOTE_LATEST=true`, also tags as `latest` (prod)
- No git-status checks or branch-name constraints
- Works identically whether source is `main`, a feature branch, or a worktree

**Dockerfile location:** `/Users/danielhuynh/Documents/sain/aura/infra/docker/Dockerfile`

**Build mechanism:**
- Stage 1: `FROM rust:1.93-slim-bookworm` builds ZeroClaw from source in `/build/`
- Stage 2: `FROM node:22-slim` runs the Aura agent (Node.js + Caddy + tini)
- Copies ZeroClaw binary, static skills, and config files
- Exposes port 8080; healthcheck via `curl http://localhost:8080/`

No gotchas detected. Script will work seamlessly with the sync worktree.

## Future: when to reconsider adding CI

Add a `.github/workflows/aura-agent-dev-image.yml` overlay if/when:
- Iteration on the sync branch exceeds ~1-2 image builds/day
- We want to produce per-commit images for automated smoke testing in M5
- Aura infra shifts to assume autonomous image artifacts (e.g., auto-reprovision on new image)

Plan 4 as originally drafted (`2026-04-23-sync-m4-ci-adoption.md`) has the workflow spec ready to adopt when that day comes.

## Related docs

- **Build script**: `/Users/danielhuynh/Documents/sain/aura/infra/ecr/build-and-push.sh`
- **Dockerfile**: `/Users/danielhuynh/Documents/sain/aura/infra/docker/Dockerfile`
- **Aura CLAUDE.md deploy section**: https://github.com/sain/aura/blob/dev/CLAUDE.md#deploy-to-production
- **Upstream CI inventory**: `.github/workflows/` in zeroclaw @ `chore/upstream-sync-v0.7.3`
