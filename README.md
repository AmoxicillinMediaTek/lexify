# Lexify Obfuscator

A Rust CLI for conservative Lua source transformation. `full_moon` validates input and emitted code; deterministic token-level passes transform local names, selected literals, truthiness tests, and unreachable source. No generated dynamic compilation is used.

## Build and use

```sh
cargo build
cargo test

cargo run --quiet -- input.lua -o output.lua --level extreme --seed 42
cargo run --quiet -- input.lua --minify --rename --encrypt-strings --obfuscate-numbers
```

CLI switches: `--level basic|medium|strong|extreme`, `--seed`, `--minify`, `--rename`, `--encrypt-strings`, `--obfuscate-numbers`, `--obfuscate-booleans`, `--obfuscate-tables`, `--control-flow`, `--dead-code`, `--target`, and `-o/--output`.

Levels combine available passes: Basic leaves transformations off; Medium enables minification and local renaming; Strong also enables number, boolean, table, and condition normalization; Extreme additionally enables string encoding and an unreachable statement. Individual flags enable those passes independently. `--target` is currently metadata only: parsing uses full_moon's combined Lua 5.1–5.4/Luau grammar, and does not guarantee a target-specific runtime library or dialect.

## Passes and safety notes

- **Minification** preserves token boundaries and removes comments/whitespace.
- **Renaming** analyzes local-like identifier bindings and closures. Lua syntax is broad and the implementation is token-based; review results for complex declaration/control-scope patterns before production use.
- **Strings** are converted into escaped byte strings and decoded at runtime using `string.byte`, a generated lookup table, and `table.concat`. This assumes those standard library functions are available and unmodified. It does not use `string.char`, `loadstring`, or `load`. Some exotic/invalid literal encodings are left unchanged.
- **Numbers** only change small non-negative decimal integer literals. Floats, hex, and large values are left unchanged.
- **Boolean/control-flow** passes make truthiness explicit using `not not`; this preserves conditional truthiness but does not restructure control-flow graphs.
- **Tables** parenthesize constructor expressions; this is a syntax-level transform, not table encryption.
- **Dead code** inserts a false-guarded local statement. It has no runtime effect in ordinary Lua.

The tool checks generated syntax. It also rejects source/output identifiers `load` and `loadstring` and a `string.char` access; these checks are deliberately conservative. Lua runtime behavior is not validated automatically unless you run the optional tests with a Lua executable in PATH.