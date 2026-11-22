# Changelog

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Nix flake with a package and an overlay.

## [0.1.1] - 2026-11-22

### Added

- Project icon, installed with the desktop entry.
- `.deb` and `.rpm` packages for x86_64 and aarch64 in each release.

## [0.1.0] - 2026-11-22

### Added

- Link handler that asks which browser, profile or private window opens each link, through rofi, fuzzel, wofi, tofi, walker, bemenu or dmenu.
- Browser and profile detection from desktop entries, Firefox and Chromium profiles.
- Rules by host glob or URL regex.
- Redirect unwrapping and tracking-parameter removal.
- Per-site suggestions ranked by frequency and recency.
- `list`, `explain`, `items`, `open`, `copy`, `forget` and `setup` commands, dynamic shell completion.
