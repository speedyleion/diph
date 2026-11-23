# Changes

## Jumping

| Key | Moves the cursor |
| --- | --- |
| `p`, `Shift+F7` | to the change before the cursor |
| `n`, `F7` | to the change after the cursor |

A jump scrolls the change into view.

## Copying

| Key | Copies one change |
| --- | --- |
| `do` | from the other side into the cursor's side |
| `dp` | from the cursor's side into the other side |

The change copied is the one the cursor's line is in, or else the one
that starts just below the cursor's line, such as lines deleted from the
cursor's side right after it. On a side's first line, a change just above
it is used when there is none at or below it. The rest of the file is
left as it is. The copy changes only the text in the window: save the
side it changed to write it to its file.
