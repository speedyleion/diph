# Marks

Marks allow one to manually align sections on the two buffers. Marking each side
with the same letter will manually align the two marks. `ma` on the left
followed by `ma` on the right will align the `a` marks between the two buffers.

The gutter will provide an indicator that there is a mark on the line.

| Key | Action |
| --- | --- |
| `m` + letter | Set a mark on the cursor's side and position |
| `d` `m` + letter | Delete the named mark on the cursor's side |
| `` ` `` + letter | Move the cursor to the mark's position |
| `'` + letter | Move the cursor to the mark's line's first non-blank character |

