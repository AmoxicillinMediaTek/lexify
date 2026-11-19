# Lexify Studio demo

A local Next.js playground for the Lexify Rust Lua obfuscator. The browser editor calls a Node.js route handler, which writes the submitted Lua to a temporary file and runs the repository's Rust CLI. Temporary source files are removed after each request.

## Requirements

- Node.js 20.9 or newer (required by Next.js 16)
- npm
- Rust toolchain (`cargo`), available on `PATH`

## Run locally

From this directory:

```sh
npm install
npm run dev
```

Open http://localhost:3000. The dev and production scripts bind to the local machine to avoid Next.js trying to enumerate network interfaces, which can fail with permission errors in restricted/mobile environments. The demo is therefore only reachable locally by default. On the first obfuscation request, Cargo may need to compile the Rust binary and fetch crates. For a production build, run `npm run build` then `npm start`.

## Demo details

- CodeMirror Lua editors with horizontal scrolling and no line wrapping.
- Select a preset, toggle individual passes, set a deterministic seed, and adjust string split size.
- `⌘/Ctrl + Enter` runs the transform. Copy the generated output from the editor header or footer.
- The API accepts at most 256 KB of source and each CLI execution has a 60-second timeout.
- The route assumes it is run from this repository's `web/` folder and uses the parent directory as the Rust project root. It is intended for local demos, not an internet-facing multi-tenant deployment. Do not expose it publicly without adding authentication, request throttling, isolated execution, and deployment-specific Rust binary provisioning.
- Source is sent to the local Next.js server for processing. It is written temporarily and removed after processing; it is not saved as a project file.
