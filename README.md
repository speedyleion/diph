# diph

A side-by-side diff tool built with [egui](https://github.com/emilk/egui).

```sh
cargo run -- <left> <right>
```

## Use with git

With `diph` on your `PATH`:

```sh
git config --global difftool.diph.cmd \
  'diph "$LOCAL" "$REMOTE" --label "$MERGED" --label "$MERGED"'
git difftool -t diph
```

Each `--label`, the first for the left side and the second for the right,
shows the file's path in the repository instead of git's temporary file, and
picks the side's syntax highlighting. Binary files print a message
instead of opening a window.

## Review comments

`c` comments on the selection, or on the cursor's line, and Ctrl+Enter
(Cmd+Enter on macOS) saves the comment. `--review <file>` saves comments in
that JSON file, keeping the comments on other paths, so one file holds a
review of every file `git difftool` opens:

```sh
git config --global difftool.diph.cmd \
  'diph "$LOCAL" "$REMOTE" --label "$MERGED" --label "$MERGED" --review "$(git rev-parse --show-toplevel)/.diph-review.json"'
```

Each comment records the path, the side, the lines it is on (and columns
for a character selection), the commented text, and the comment.
