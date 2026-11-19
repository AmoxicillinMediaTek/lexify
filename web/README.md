# Lexify Studio demo

A static Next.js playground for the Lexify Rust Lua obfuscator. The Rust transformation library is compiled to WebAssembly and runs entirely in the browser; submitted Lua source is not sent to a server.

## Requirements

- Node.js 20.9 or newer (required by Next.js 16)
- npm
- Rust toolchain (`cargo`) and `wasm-pack` — only needed to rebuild the browser module

## Run locally

The UI loads `public/wasm/lexify_obfuscator.js` and `public/wasm/lexify_obfuscator_bg.wasm` at runtime. That directory is gitignored, so populate it first, either by downloading the `lexify-wasm` artifact from the latest run of the [Deploy to Vercel](../.github/workflows/deploy-vercel.yml) workflow, or by building it on a machine with the Rust toolchain:

```sh
# from the repository root
rustup target add wasm32-unknown-unknown
wasm-pack build . --target web --release --no-pack \
  --out-dir web/public/wasm --out-name lexify_obfuscator
```

Then start the site:

```sh
cd web
npm install
npm run dev
```

Open http://localhost:3000. `npm run build` creates the static export in `out/`. The development and production scripts bind to localhost by default.

## Demo details

- CodeMirror Lua editors with horizontal scrolling and no line wrapping.
- Select a preset, toggle individual passes, set a deterministic seed, and adjust string split size.
- `⌘/Ctrl + Enter` runs the transform. Copy the generated output from the editor header or footer.
- Obfuscation runs in the user's browser via WebAssembly; no server-side API or Rust runtime is needed after the static site is built.
- Set `NEXT_PUBLIC_BASE_PATH` when the site is not served from the domain root.

## Deployment

`.github/workflows/deploy-vercel.yml` owns the deployment. On every push to `main` it installs Rust and `wasm-pack`, builds the WebAssembly module into `web/public/wasm`, then runs `vercel pull`, `vercel build --prod` and `vercel deploy --prebuilt --prod`. Vercel never compiles Rust; it only hosts the prebuilt output.

One-time setup:

1. Create a Vercel token at <https://vercel.com/account/tokens>.
2. Link the project once from the repository root (`vercel link`) and copy `orgId` and `projectId` from `.vercel/project.json`.
3. Add three GitHub repository secrets: `VERCEL_TOKEN`, `VERCEL_ORG_ID`, `VERCEL_PROJECT_ID`.
4. In the Vercel project settings, set **Root Directory** to `web` and **Build Command** to `npm run build`, since the WebAssembly step already ran in CI.
5. Turn off Vercel's own Git deployments (Settings → Git), or add an ignored build step, so that CI is the only thing that deploys.

The Vercel CLI commands must run from the repository root, not from `web/`: the CLI appends the configured Root Directory itself, so running them inside `web/` resolves to `web/web`.