# Packaging & distribution

Everything flows from **one event**: pushing a `vX.Y.Z` tag. That builds the
cross-platform binaries + checksums, creates the GitHub Release, publishes to
crates.io, and attaches `.deb`s. Downstream packages point back at those release
assets.

```
git tag → GitHub Release (binaries + .sha256 + .deb) → crates.io
                                   ↓
        binstall · Nix · Homebrew · AUR · Scoop · WinGet · COPR · Snap · …
```

## Tiers (do them in this order)

### Tier 1 — self-serve, already wired up (no accounts, no PRs)

| Channel | User command | Status |
| --- | --- | --- |
| crates.io | `cargo install netchecker` | ✅ live |
| cargo-binstall | `cargo binstall netchecker` | ✅ `[package.metadata.binstall]` in Cargo.toml |
| Nix flake | `nix run github:pourmand1376/netchecker` | ✅ `flake.nix` |
| `.deb` (download) | `sudo apt install ./netchecker_*.deb` | ✅ built + attached by CI |

Nothing to do here except cut a release. Verify after the next tag:
- `cargo binstall netchecker`
- `nix run github:pourmand1376/netchecker -- direct`
- download the `.deb` from the Release and `dpkg -i`.

### Tier 2 — self-serve but needs a one-time repo/account

**Homebrew tap** — `brew install pourmand1376/tap/netchecker`
1. Create repo `pourmand1376/homebrew-tap`.
2. Easiest: adopt [`dist`](https://github.com/axodotdev/cargo-dist) for the
   formula (it keeps the SHA256s correct automatically):
   ```sh
   cargo install cargo-dist
   dist init            # choose: homebrew, shell, powershell installers
   ```
   In `Cargo.toml` set:
   ```toml
   [workspace.metadata.dist]
   installers = ["homebrew", "shell", "powershell"]
   tap = "pourmand1376/homebrew-tap"
   publish-jobs = ["homebrew"]
   ```
   `dist` generates its own release workflow that updates the tap on each tag.
   (Alternative: hand-write a formula, but then you update the SHA256 every
   release — don't, unless you enjoy it.)

**AUR** — `yay -S netchecker-bin`
1. Make an account at https://aur.archlinux.org and add your SSH key.
2. Per release, from `packaging/aur/`:
   ```sh
   # bump pkgver to match the tag, then:
   updpkgsums
   makepkg --printsrcinfo > .SRCINFO
   git clone ssh://aur@aur.archlinux.org/netchecker-bin.git aur && cd aur
   cp ../PKGBUILD ../.SRCINFO .
   git commit -am "netchecker-bin X.Y.Z" && git push
   ```

**Scoop (Windows)** — `scoop install netchecker`
Create a bucket repo `pourmand1376/scoop-bucket` with a `netchecker.json`
pointing at the Windows `.zip` release asset + its SHA256 (autoupdate block can
track new versions).

### Tier 3 — gated by maintainer review (can't be automated away)

These have real review queues; submit once and they update via their own bots.

- **nixpkgs** (`nix profile install nixpkgs#netchecker`): PR a
  `rustPlatform.buildRustPackage` expression to NixOS/nixpkgs.
- **Homebrew core** (`brew install netchecker`, no tap): PR to homebrew-core
  once the project is notable. The tap covers you until then.
- **Fedora COPR** → later a Fedora review: build RPM with `cargo-generate-rpm`,
  publish via COPR (`dnf copr enable pourmand1376/netchecker`).
- **WinGet** (`winget install ...`): PR manifests to microsoft/winget-pkgs.
- **Snap** (`snap install netchecker`): `snapcraft.yaml` + Snap Store upload.
- **Ubuntu PPA** (`add-apt-repository ppa:...`): Launchpad source package.
- **openSUSE OBS**: can build RPM/DEB for many distros from one source.
- **Alpine aports**, **Gentoo GURU**, **MacPorts**, **Chocolatey**: per-distro
  recipes, PR to each.
- **Debian official**: hardest — needs a Debian Developer sponsor. Long game.

## What lives in this repo

- `Cargo.toml` → `[package.metadata.binstall]`, `[package.metadata.deb]`
- `flake.nix` → Nix build (version auto-read from Cargo.toml)
- `.github/workflows/release.yml` → binaries, checksums, `.deb`, crates.io
- `packaging/aur/PKGBUILD` → AUR `-bin` recipe
