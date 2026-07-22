---
status: pending
priority: p2
issue_id: "002"
tags: [release, distribution, github-actions]
dependencies: []
---
# Package Standalone Runnables in GitHub Releases

## Problem Statement

GitHub Releases do not currently provide prebuilt versions of the game. Players must clone the repository, install Rust, and run `cargo run -p game-play`.

Add a tag-driven release pipeline that publishes platform-specific runnable archives. A release should not require Rust or Cargo and should clearly document that the game runs in a terminal of at least `160x45` cells.

## Findings

- `.github/workflows/ci.yml` validates pushes and pull requests but does not build or publish release assets.
- `crates/game-play/Cargo.toml` produces the `4x-term` binary.
- `crates/game-play/src/main.rs` currently loads `content/profiles/starter.ron` relative to the process working directory.
- Shipping only the current executable would therefore omit a required default profile.
- Shipping an executable plus `content/` works only when the process is started with the expected working directory unless startup path resolution is changed.
- A true single-file executable could embed the starter profile while retaining support for user-selected external profile files.
- Likely initial targets are Windows x64, Linux x64, macOS Apple Silicon, and macOS Intel. A MUSL Linux build would improve portability across distributions.
- Unsigned Windows and macOS binaries may produce operating-system security warnings. Signing and macOS notarization can be deferred but should be documented.

## Proposed Solution

**Approach:**
- Decide whether the release contract is a single executable or a self-contained extracted directory.
- Prefer embedding the default starter profile for a true standalone executable, while preserving external editable profile selection.
- Add a tag-triggered `.github/workflows/release.yml` matrix that builds `game-play` with `cargo build --locked --release -p game-play`.
- Package one archive per supported target, include `README.md` and `LICENSE`, generate SHA-256 checksums, and attach the assets to a GitHub Release.
- Test packaged artifacts from outside the repository and outside the extraction directory where applicable.

**Why this approach:**
- Players receive a native runnable without installing the Rust toolchain.
- Tag-triggered automation keeps release artifacts tied to an exact source revision.
- Embedding the default profile removes the current working-directory dependency without preventing advanced users from loading custom profiles.

**Trade-offs / risks:**
- Supporting four targets increases workflow complexity and runner time.
- Linux target selection must balance static portability against target/toolchain setup.
- macOS signing/notarization and Windows signing require credentials and may be separate follow-up work.
- Embedded content requires a clear precedence rule between the built-in default and external profiles.

## Recommended Action

1. Choose the initial platform matrix and whether releases promise a single executable or an extracted standalone directory.
2. Change default profile loading so a packaged game does not depend on the repository or caller working directory.
3. Add automated coverage for default-profile startup in the chosen packaged arrangement.
4. Add and validate the tag-triggered release workflow using a draft or prerelease tag.
5. Document download, extraction, terminal launch, minimum terminal dimensions, custom profile usage, checksums, and unsigned-binary warnings.
6. Publish the first release only after testing every uploaded archive on its target platform.

## Technical Details

**Affected files/assets:**
- `crates/game-play/src/main.rs` - remove the repository-relative default-profile runtime assumption.
- `crates/game-app/src/lib.rs` - may need an embedded/default profile source representation or fallback contract.
- `content/profiles/starter.ron` - default profile to embed or bundle.
- `.github/workflows/release.yml` - new build, package, checksum, and GitHub Release workflow.
- `README.md` - add binary release installation and launch instructions.
- `CHANGELOG.md` - record user-visible packaged release support under `Unreleased`.

**Related systems:**
- Game process composition and startup coordination
- Content/profile loading
- GitHub Actions and release distribution

**Data/content impact:**
- Save data affected? No; persistence is not currently implemented.
- Serialized assets or prefabs affected? No.
- Migration or content reimport needed? No.

## Resources

- **Review/PR/changeset:** None yet
- **Related issue/card:** None
- **Log/capture:** None
- **Documentation:** `README.md`, `docs/architecture.md`
- **Similar pattern:** `.github/workflows/ci.yml`

## Acceptance Criteria

- [ ] The release contract explicitly chooses single executable or self-contained extracted directory behavior.
- [ ] A packaged build starts without Rust, Cargo, the source repository, or an assumed repository working directory.
- [ ] The default starter profile remains available, and external custom profiles still work if that workflow is retained.
- [ ] A version tag triggers locked release builds for the approved platform matrix.
- [ ] GitHub Release assets use clear version/platform/architecture names.
- [ ] Windows assets contain `4x-term.exe`; Unix-like assets contain `4x-term` with executable permissions preserved.
- [ ] Linux portability is validated for the selected target, preferably `x86_64-unknown-linux-musl` if dependencies support it.
- [ ] Release assets include `README.md`, `LICENSE`, and published SHA-256 checksums as appropriate to the chosen packaging contract.
- [ ] Every archive is smoke-tested on its target platform from a clean location outside the repository.
- [ ] Release documentation explains terminal launch, the `160x45` minimum, custom profiles, and expected unsigned-binary warnings.
- [ ] Existing format, check, Clippy, and workspace test commands pass.
- [ ] `CHANGELOG.md` is updated under `Unreleased`.

## Work Log

### 2026-07-21 - Initial Release Packaging Investigation

**By:** Pi coding assistant

**Actions:**
- Reviewed the workspace manifest, binary package, CI workflow, README, and architecture documentation.
- Identified the repository-relative starter-profile path as the primary obstacle to a standalone executable.
- Outlined platform archives, tag-triggered GitHub Actions builds, and checksum publication.

**Learnings:**
- The Rust binary is already isolated in `game-play`; distribution work primarily concerns profile loading and release automation.
- A bundled directory is possible immediately, but robust startup requires either executable-relative lookup or an embedded default profile.

## Notes

- Start with a draft or prerelease to verify asset contents before making the release public.
- Do not store code-signing credentials in the repository.
- Keep release packaging separate from the existing push/pull-request CI workflow unless a later design deliberately combines them.
