# diph

A side-by-side diff tool that supports local review comments.

## Use with git

```sh
git config --global difftool.diph.cmd \
  'diph "$LOCAL" "$REMOTE" --label "$MERGED" --label "$MERGED" --review "$(git rev-parse --show-toplevel)/.diph-review.json"'
```

The `--label` argument specifies the filename to display on each side's title
bar, the first for the left side and the second for the right. This can be used
to show a file's path in the repository instead of git's temporary file.

## Review comments

Pressing `c` in normal mode will start a review comment at the cursor position.
Enter or the Comment button saves the comment, and Shift+Enter starts a new line
in it. Escape or the Cancel button discards it. Pressing `c` on a commented line
edits the comment. Saving an empty comment delets it.
`--review <file>` specifies a file to save review comments to. Comments from
multiple files can be written to the same review file. Re-opening a file in
`diph` and adding or removing comments will replace all comments in the review
file for the file(s) being diffed.

`]c` and `[c` move to the next and previous comment
