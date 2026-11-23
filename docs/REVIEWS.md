# Review Comments

Pressing `c` in normal mode opens a comment box under the cursor position.
While in the comment box the keyboard behaves like in insert mode.

One can also select multiple lines to create a comment relating to the entire
selection.

`c` on a line that already has a comment opens that comment to edit. 

## Comment Mode Keybindings

| Key | action |
| --- | --- |
| `Enter` | save the comment and return to normal mode |
| `Escape` | discard modifications and return to normal mode |

Saving an empty comment deletes it.
Comments are written to the file named by the `--review` command line argument.

## Navigating Comments

| Key | Opens or moves a comment |
| --- | --- |
| `c` | a comment box on the selection, or the cursor's line |
| `]c` | to the next comment |
| `[c` | to the previous comment |

