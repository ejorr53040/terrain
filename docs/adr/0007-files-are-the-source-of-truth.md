# Files are the source of truth; blocks are structure, not syntax

terrain-blocks shows a Rust crate as blocks parsed from the files on disk, and a move rewrites those files at once. Nothing else is stored apart from where file blocks sit on the canvas (the sidecar). Blocks stop at the item level (files > inline modules, `impl`s, `trait`s > items), so they show how code is organised, not what it says. This keeps git, cargo, nvim and other editors fully in charge of the code, and lets the app open any crate without importing or exporting it.

## Considered options

- A block project format that generates Rust: blocks could hold anything, but the code would have two owners and would drift from what's on disk.
- Scratch-style syntax blocks below the item level: toys at real-project scale. Editing statements is nvim's job.
- `syn` for parsing: drops comments, so a move would lose them. tree-sitter keeps byte ranges and comments.

## Consequences

- A move only moves text. `use` paths and call sites are not fixed; the `cargo check` badge shows when a move broke something.
- Edits made outside the app win: a move is refused if a file it touches changed on disk since it was read, and changed files are re-read about every second.
- Undo is an in-memory stack of file text, and git is the deeper undo.
