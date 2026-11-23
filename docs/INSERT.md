# Insert mode

## Entering insert mode

| Key | Puts the cursor in insert mode |
| --- | --- |
| `i` | before the character under the cursor |
| `a` | after the character under the cursor |
| `I` | at the first non-blank character of the line |
| `A` | at the end of the line |
| `o` | on a new line below the cursor |
| `O` | on a new line above the cursor |

## In insert mode

In insert mode the arrow keys move the cursor and every other character is
typed into the side.

| Key | In insert mode |
| --- | --- |
| `←` `→` `↑` `↓` | move the cursor |
| `Escape` | leave insert mode |

Leaving insert mode moves the cursor back over the character to its left, as in vim.
