---
name: navigate-code
description: Map an unfamiliar codebase with the analyzer — outline, definitions, references — instead of re-reading whole files.
---

# Navigate code

Find the relevant code by asking the analyzer, not by re-reading files top to
bottom (that burns context, and the no-progress guard will nudge you):

1. **Shape a file fast (`lsp_symbols <file>`).** The analyzer's outline —
   functions/structs/impls with line numbers — beats reading the whole file or the
   regex `outline`/`defs`. Pull only the symbols you actually need.
2. **Follow a symbol to its source (`lsp_definition <file> symbol=<name>`).** Jump
   straight to where something is defined instead of grepping and guessing which
   match is the declaration.
3. **See every caller before you change it (`lsp_references <file> symbol=<name>`).**
   The list of uses IS the blast radius of your edit — check it before touching a
   shared function or type.
4. **Confirm a type/contract at a point (`lsp_hover`).** Verify a value's actual
   type or a function's real signature instead of inferring it.
5. **Discovery → search; navigation → analyzer.** Use `grep`/`file_search` to find
   *where* to start (a string, a filename), and `find_files`/`list_dir` for the
   lay of the land; then switch to the analyzer tools above to move precisely.
6. **Page source when reading is necessary.** `read_file(path)` returns a bounded
   first page. For a large file, follow its printed `offset` with
   `read_file(path, offset, limit)` instead of asking for the whole file again;
   this keeps the next model hop focused on one contiguous range.

(The `lsp_*` tools need `ANGEL_LSP`; without it, fall back to `outline`/`defs` +
`grep`, but the analyzer is far more precise.)

Smell: reading the same file three times. You already have it — outline it once
(`lsp_symbols`), then jump (`lsp_definition`) to the one place that matters.
