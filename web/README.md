# aozora-proof web app

The static front end for [aozora-proof](https://github.com/P4suta/aozora-proof) —
青空文庫記法テキストの文字レベル校正 (`/`) and 外字検索 (`/gaiji`). SvelteKit +
Tailwind CSS, prerendered to fully static files by `@sveltejs/adapter-static`,
and published to [GitHub Pages](https://p4suta.github.io/aozora-proof/).

The checks come from the `aozora-proof-wasm` crate, built with `wasm-pack` into
`src/lib/pkg/` (git-ignored) and loaded in the browser.

## Develop

From the repository root (needs Node 24+, pnpm, and `wasm-pack`):

```sh
pnpm -C web install   # or: just setup-web
just serve            # builds the wasm, runs the Vite dev server at :5173
just web              # same, plus rebuilds the wasm on Rust changes
```

`just lint-web` runs the full gate: `wasm-pack` build, then `pnpm run lint`
(prettier + eslint), `pnpm run check` (svelte-check), and `pnpm run build`
(prerender). The page set is whatever CI deploys — see `.github/workflows/docs.yml`.
