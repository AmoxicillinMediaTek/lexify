# Lexify Obfuscator

A Rust CLI for conservative Lua source transformation. `full_moon` validates input and emitted code; deterministic token-level passes transform local names, selected literals, truthiness tests, and unreachable source. No generated dynamic compilation is used.

## Build and use

```sh
cargo build
cargo test

cargo run --quiet -- input.lua -o output.lua --level extreme --seed 42
cargo run --quiet -- input.lua -o output.lua --split-strings --lookup-tables --randomize-lookup --shared-lookup --split-size 3 --seed 42
cargo run --quiet -- input.lua --minify --rename --encrypt-strings --obfuscate-numbers
```

CLI switches: `--level basic|medium|strong|extreme`, `--seed`, `--minify`, `--rename`, `--encrypt-strings`, `--obfuscate-numbers`, `--obfuscate-booleans`, `--obfuscate-tables`, `--control-flow`, `--dead-code`, `--split-strings`, `--lookup-tables`, `--randomize-lookup`, `--shared-lookup`, `--split-size N`, `--target`, and `-o/--output`.

Levels combine available passes: Basic leaves transformations off; Medium enables minification and local renaming; Strong also enables number, boolean, table, and condition normalization; Extreme additionally enables string encoding and an unreachable statement. Individual flags enable those passes independently. `--target` is currently metadata only: parsing uses full_moon's combined Lua 5.1–5.4/Luau grammar, and does not guarantee a target-specific runtime library or dialect.

## Passes and safety notes

- **Minification** preserves token boundaries and removes comments/whitespace.
- **Renaming** analyzes local-like identifier bindings and closures. Lua syntax is broad and the implementation is token-based; review results for complex declaration/control-scope patterns before production use.
- **Strings**: `--encrypt-strings` converts literals into escaped byte strings and decodes them at runtime using `string.byte`, a generated lookup table, and `table.concat`. `--split-strings` instead parses and rewrites AST string-literal expressions into ordered byte fragments; `--lookup-tables` stores fragments in generated tables. Add `--randomize-lookup` to randomize fragment sizes and table keys, `--shared-lookup` to use one table across the source, and `--split-size N` to set the maximum fragment length in bytes. Seeded output is deterministic. These paths preserve byte values, including UTF-8 and binary strings, and generated code does not use `string.char`, `loadstring`, or `load`. Splitting is AST-backed and covers literal expression positions including call arguments, table fields, and nested expressions. It assumes standard Lua table/string behavior and leaves unsupported literal encodings unchanged.

- **Numbers** only change small non-negative decimal integer literals. Floats, hex, and large values are left unchanged.
- **Control-flow** wraps eligible `if` and `while` conditions in `not not (...)`; this preserves conditional truthiness and does not reorder blocks.

- **Tables** parenthesize constructor expressions; this is a syntax-level transform, not table encryption.
- **Dead code** inserts a false-guarded local statement. It has no runtime effect in ordinary Lua.

`--level extreme` enables AST-backed string splitting with randomized shared lookup tables. Explicit `--encrypt-strings` combined with `--split-strings` uses the AST fragment encoding path rather than applying the older decoder to generated fragments. Local renaming happens before the AST rewrite so generated lookup names are not mistaken for source bindings. Runtime equivalence is tested with `lua5.4` when available; `tests/fixtures/strings.lua` covers edge cases, while integration tests verify behavior and deterministic seeded output.