# Lexify Studio demo

A static Next.js playground for the Lexify Rust Lua obfuscator. The Rust transformation library is compiled to WebAssembly and runs entirely in the browser; submitted Lua source is not sent to a server.

## Requirements

- Node.js 20.9 or newer (required by Next.js 16)
- npm
- Rust toolchain (`cargo`) and `wasm-pack` to build the browser module

## Run locally

From this directory:

```sh
npm install
npm run build:pages
npm run dev
```

Open http://localhost:3000. `npm run build:pages` compiles the Rust library to browser-targeted WebAssembly and then creates the static site export in `out/`. The development and production scripts bind to localhost by default.

## Demo details

- CodeMirror Lua editors with horizontal scrolling and no line wrapping.
- Select a preset, toggle individual passes, set a deterministic seed, and adjust string split size.
- `⌘/Ctrl + Enter` runs the transform. Copy the generated output from the editor header or footer.
- Obfuscation runs in the user's browser via WebAssembly; no server-side API or Rust runtime is needed after the static site is built.
- The Pages workflow publishes the contents of `out/` and configures the repository base path automatically.
