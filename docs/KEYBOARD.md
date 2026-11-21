# Keyboard commands

diph shows the two sides of a diff in one window. The cursor sits on one side
at a time, and the side it is on is the side that gets edited, saved, or
commented on.

diph has three modes, each shown in the status bar: **normal** (no label),
**insert** (`-- INSERT --`), and **comment** (`-- COMMENT --`). A key acts
differently in each mode, so the commands are listed by mode.

## Normal mode

### Movement

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

The view scrolls to keep the cursor in sight. A number typed before a
command repeats it: `10j` moves down ten lines. The number also picks the
line for `gg` and `G` (`5gg` jumps to line 5) and how many comments `]c`
and `[c` skip. A `0` is part of a number only after another digit.

### Changes

| Key | Jumps the cursor |
| --- | --- |
| `p`, `Shift+F7` | to the change before the cursor |
| `n`, `F7` | to the change after the cursor |

A jump scrolls the change into view.

### Selection

| Key | Starts or cancels a selection |
| --- | --- |
| `v` | a character selection, at the cursor |
| `V` | a line selection, at the cursor |
| `Escape` | cancels a selection |
| `y` | copies the selection to the clipboard |

Move the cursor to extend a selection.

### Comments

| Key | Opens or moves a comment |
| --- | --- |
| `c` | a comment box on the selection, or the cursor's line |
| `]c` | to the next comment |
| `[c` | to the previous comment |

With no selection, `c` on a line that already has a comment opens that
comment to edit. Save or discard the comment in the comment mode table
below.

### Enter insert mode

| Key | Puts the cursor in insert mode |
| --- | --- |
| `i` | before the character under the cursor |
| `a` | after the character under the cursor |
| `I` | at the first non-blank character of the line |
| `A` | at the end of the line |
| `o` | on a new line below the cursor |
| `O` | on a new line above the cursor |

### File and window

| Key | Acts on the side or the window |
| --- | --- |
| `Ctrl+s` | saves the side the cursor is on |
| `F5` | reloads the files |
| `ZZ` | closes the window |

On macOS these are `Cmd+s` and `Cmd+r`; see Platform differences. Saving
writes the side the cursor is on to its file, and reloading re-reads both
files. Both are also on the **File** menu, with the same shortcuts.

## Insert mode

In insert mode the arrow keys move the cursor and every other character is
typed into the side.

| Key | In insert mode |
| --- | --- |
| `←` `→` `↑` `↓` | move the cursor |
| `Escape` | leave insert mode |

Leaving insert mode moves the cursor back over the character it left, as in vim.

## Comment mode

Pressing `c` in normal mode opens a comment box under the selection, or
under the cursor's line. Type the comment into the box.

| Key | In comment mode |
| --- | --- |
| `Enter` | save the comment and return to normal mode |
| `Escape` | discard modifications and return to normal mode |

Saving an empty comment deletes it.
Comments are written to the file named by `--review`.

## Platform differences

Three shortcuts differ on macOS, where `Cmd` takes the place of `Ctrl`.

| Action | macOS | Else |
| --- | --- | --- |
| Reload | `Cmd+r` | `F5` |
| Save a comment | `Cmd+Enter` | `Ctrl+Enter` |
