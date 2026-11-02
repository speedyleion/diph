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
