import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

export default defineConfig({
  // The site is published at kantord.github.io/toylang/, so assets resolve under the repo name
  // rather than the domain root. `pnpm dev` overrides nothing: Vite serves this prefix locally
  // too, which is what stops a path working in development and 404ing once deployed.
  base: "/toylang/",
  // annotationsInbox()/grillRounds()/grillForest() moved to vite.tools.config.ts (`pnpm
  // dev:tools`): this config's dev server is a prerendered static docs site that never calls
  // `/__annotations/*` or `/__grill*` at runtime, so it doesn't need to serve them either.
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { "@": import.meta.dirname + "/src", "@dev": import.meta.dirname + "/dev/src" },
  },
  // No `build.rollupOptions.input` here: Vite's default build entry is just this directory's
  // index.html, so dev/index.html and everything under dev/ (kantord/toylang#50 -- mail app,
  // grill wizard, board, annotations mode) is never in `vite build`'s module graph at all. That
  // is the tree-shaking guarantee now: a directory a production build never opens, not an
  // `import.meta.env.DEV` check a future edit could forget.
  // The docs pages live at the repository root, beside the code and the harness that runs
  // their fragments, so the dev server must be allowed to read one level up.
  server: {
    fs: { allow: [import.meta.dirname + "/.."] },
  },
})
