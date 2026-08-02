# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0-alpha.1] - 2026-08-02

First released build. Alpha: it speaks a protocol reconstructed by observation and has been
run against exactly one headset.

### Added

- **An installer.** A per-user setup executable with a Start menu entry, published on the
  releases page. No administrator rights, no driver, no service.
- **An application icon.** The headset the tray draws, embedded in the executable at five
  sizes, so Explorer, the Start menu, and the shortcut all show it.

- Noise control: off, ANC, or ambient, with an ANC level of 1–4. Exposed as `headsetctl
  noise` and as a segmented control in the tray panel. Parameter `0x12`, identified by
  capture — see `docs/device-research.md`.
- The tray icon can be operated from the keyboard: focus it and press Enter or Space.
- A high-contrast palette, applied when Windows high contrast is on.
- A stable notification-icon identity, so a pin-to-taskbar choice survives reinstalling.
- A Direct2D tray panel, replacing the context-menu-only interface.

### Fixed

- The tray icon no longer disappears permanently when Explorer restarts.
- The panel opens on the monitor its icon is on, rather than being clamped to the primary
  display's work area.
- The panel renders at the display's real pixel density instead of being stretched.
- The panel is placed clear of a taskbar docked to the left or right, instead of over it.
- A second launch raises the running instance instead of adding a duplicate tray icon.
- The panel holds its bottom edge when its height changes, instead of growing downward
  over the taskbar.
- Changing a noise setting updates the panel immediately rather than a beat later.

### Changed

- Dual-licensed under MIT OR Apache-2.0. Previously unlicensed and all rights reserved.
