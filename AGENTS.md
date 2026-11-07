# diph

## Commit messages and PR descriptions

PRs are squash-merged with the PR title and description as the commit
message, so write both for someone reading `git log` on main a year from
now.

- Describe what the change does and why, as it stands once merged.
- Stay at the level of behavior. Leave algorithms, data structures, and
  examples of how the code works to the diff and the tests.
- Don't mention future work, follow-up PRs, or what will come later.
- Don't mention what the change doesn't do, or work that may not exist
  later (spikes, other branches, other PRs).
- Don't say where an idea came from ("modelled on X") unless the reader
  needs it to understand or maintain the code.
- Don't narrate how the PR evolved ("Also:", "after review"). Update the
  description so it matches the final diff.
- Title: imperative and under ~70 characters. Body: wrap at 72 columns.

## Code comments, docs, and descriptions

The same reader applies to code comments, the README, crate descriptions,
and `--help` text: describe what the code does now.

- Keep descriptions to a plain summary of what the tool does. Leave out
  adjectives that don't tell the reader anything ("native") and features
  that don't exist yet ("images").
- Don't describe the project's status or plans ("early in development",
  "does not do X yet").
- Don't reference how the code is developed or reviewed, such as stacked
  PRs or branches, in comments or config.
- A doc comment states what a caller gets: inputs, outputs, and
  guarantees. Leave out how the function computes it and what callers do
  with the result.
- Don't repeat what the signature already says, such as the return type
  or that there are two of something.

## Code

- Before writing an algorithm by hand, check whether an existing
  dependency already provides it.
- Name fields after what they hold (`deleted_words`, not `old_changed`).
- Use a struct with named fields, not a tuple, for data that is stored,
  sent between threads, or given a type alias. Keep tuples for values
  destructured right away, such as an iterator's pairs.
- When a nested collection's layers mean different things, wrap a layer
  in a named type (`SideColors`, not `Vec<Vec<Span>>`).
- Prefer iterator chains and `collect()` to `for` loops over `mut`
  accumulators when the result reads as clearly.
- Keep closures to a couple of lines. Make anything longer a named
  function, passing in what it needs from the enclosing scope.
- Don't wrap an expression in parentheses that only change where
  rustfmt breaks the line, such as `(rows.iter().enumerate())`.
- Don't write a function that only passes its arguments on to a struct
  literal or another function. Write the literal or call where it's
  needed.

## Tests

- Give each test one arrange, act, and assert. When another case needs
  its own setup and call, make it a separate case instead of continuing
  the same test.
- Cover several inputs to one behavior with a `yare::parameterized`
  test, naming each case for what it shows.
- Keep the tests of one behavior together. Add tests for a new behavior
  after the existing tests, not between tests of another behavior.
