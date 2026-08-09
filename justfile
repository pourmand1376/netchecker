# netchecker task runner. Run `just` to list recipes.
# Requires: just, cargo. Version recipes also need cargo-edit (`just tools`).

set shell := ["bash", "-uc"]

bin := "netchecker"

# List available recipes.
default:
    @just --list

# ---------------------------------------------------------------------------
# Everyday
# ---------------------------------------------------------------------------

# Run the app, forwarding args, e.g. `just run direct`.
run *ARGS:
    cargo run -- {{ ARGS }}

# Debug build.
build:
    cargo build

# Optimized release build.
release-build:
    cargo build --release

# Run tests.
test:
    cargo test

# Type-check without producing binaries.
check:
    cargo check --all-targets

# Format the code.
fmt:
    cargo fmt --all

# Verify formatting (CI does this).
fmt-check:
    cargo fmt --all --check

# Lint with clippy, warnings-as-errors (CI does this).
lint:
    cargo clippy --all-targets -- -D warnings

# Everything CI runs, locally.
ci: fmt-check lint test

# Audit dependencies for known vulnerabilities (needs cargo-audit).
audit:
    cargo audit

# Install netchecker into ~/.cargo/bin from source.
install:
    cargo install --path .

# Remove build artifacts.
clean:
    cargo clean

# Update dependencies within semver.
update:
    cargo update

# Cross-build a single release target locally, e.g.
# `just dist x86_64-unknown-linux-musl` (needs the target/toolchain installed).
dist TARGET:
    cargo build --release --target {{ TARGET }}

# Install the dev tooling these recipes rely on.
# cargo-edit provides `cargo set-version` (used by the version-* recipes).
tools:
    cargo install cargo-edit cargo-audit

# ---------------------------------------------------------------------------
# Versioning & release
#
# These bump the version in Cargo.toml, commit, tag `vX.Y.Z`, and push.
# Pushing the tag triggers .github/workflows/release.yml, which builds the
# cross-platform binaries AND runs `cargo publish` to crates.io.
# ---------------------------------------------------------------------------

# Bump patch and release, e.g. 1.2.3 -> 1.2.4  (cargo set-version --bump patch)
version-patch:
    @just _release patch

# Bump minor and release, e.g. 1.2.3 -> 1.3.0  (cargo set-version --bump minor)
version-minor:
    @just _release minor

# Bump major and release, e.g. 1.2.3 -> 2.0.0  (cargo set-version --bump major)
version-major:
    @just _release major

# Shared bump/commit/tag/push. LEVEL is patch|minor|major.
_release LEVEL:
    #!/usr/bin/env bash
    set -euo pipefail

    # Refuse to run on a dirty tree so the release commit is just the bump.
    if ! git diff-index --quiet HEAD -- || [ -n "$(git status --porcelain --untracked-files=no)" ]; then
        echo "error: working tree has uncommitted changes — commit or stash first." >&2
        exit 1
    fi

    # Make sure the tooling is present.
    if ! cargo set-version --help >/dev/null 2>&1; then
        echo "error: cargo-edit not installed. Run: just tools" >&2
        exit 1
    fi

    cargo set-version --bump "{{ LEVEL }}"
    version="$(cargo pkgid | sed -E 's/.*[#@]//')"

    git add Cargo.toml Cargo.lock
    git commit -m "release: v${version}"
    git tag -a "v${version}" -m "netchecker v${version}"

    echo "Tagged v${version}. Pushing to trigger the release pipeline..."
    git push origin HEAD --follow-tags
    echo "Done. Watch: https://github.com/pourmand1376/netchecker/actions"
