# Motions

## Movement

| Key | Moves the cursor |
| --- | --- |
| `h`, `←` | left one character |
| `l`, `→` | right one character |
| `j`, `↓` | down one line |
| `k`, `↑` | up one line |
| `Ctrl+d` | down half a page |
| `Ctrl+u` | up half a page |
| `Ctrl+f` | down a page |
| `Ctrl+b` | up a page |
| `gg` | to the first line |
| `G` | to the last line |
| `^` | to the first non-blank character of the line |
| `$` | to the end of the line |
| `0` | to the first character of the line |
| `zz` | to the center of the view |
| `zt` | to the top of the view |
| `zb` | to the bottom of the view |
| `Ctrl+w` `h` | to the left side |
| `Ctrl+w` `l` | to the right side |
| `Ctrl+w` `Ctrl+w` | to other side |

The view scrolls to keep the cursor in sight. A number typed before a
command repeats it: `10j` moves down ten lines. The number also picks the
line for `gg` and `G` (`5gg` jumps to line 5).
