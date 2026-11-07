# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- A cursor, placed by a click. h, j, k, and l or the arrow keys move it a
  character or a line, gg and G to the first and last line, and Ctrl+W h
  and Ctrl+W l to the left and right side. The view scrolls to keep it in
  sight, and stepping to a change moves it there.

### Changed

- j and k move the cursor instead of scrolling the view

## [0.1.2] - 2026-11-06

### Changed

- The way the way the title bar is displayed

### Added

- Startup time in status bar
- j and k scroll down and up a line, gg and G to the top and bottom
- A change map beside the diff marks where the changes are and which rows
  are in view

## [0.1.1] - 2026-11-05

### Added

- two-face dependency for more syntax highlighting dependencies
- Add third party licenses to release archives
- Save and restore window size and position

## [0.1.0] - 2026-11-05

Initial release

[unreleased]: https://github.com/speedyleion/gh-difftool/releases/tag/v0.1.2...HEAD
[0.1.2]: https://github.com/speedyleion/gh-difftool/releases/tag/v0.1.2
[0.1.1]: https://github.com/speedyleion/gh-difftool/releases/tag/v0.1.1
[0.1.0]: https://github.com/speedyleion/gh-difftool/releases/tag/v0.1.0
