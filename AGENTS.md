# diph

## Commit messages and PR descriptions

PRs are squash-merged with the PR title and description as the commit
message, so write both for someone reading `git log` on main a year from
now.

- Describe what the change does and why, as it stands once merged.
- Don't mention future work, follow-up PRs, or what will come later.
- Don't mention what the change doesn't do, or work that may not exist
  later (spikes, other branches, other PRs).
- Don't say where an idea came from ("modelled on X") unless the reader
  needs it to understand or maintain the code.
- Don't narrate how the PR evolved ("Also:", "after review"). Update the
  description so it matches the final diff.
- Title: imperative and under ~70 characters. Body: wrap at 72 columns.
