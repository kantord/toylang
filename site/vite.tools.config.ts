import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig, type Plugin } from "vite"

import { annotationsInbox } from "./vite-plugins/annotations-inbox.ts"
import { grillForest } from "./vite-plugins/grill-forest.ts"
import { grillRounds } from "./vite-plugins/grill-rounds.ts"

// The dev-only tooling's own Vite config (kantord/toylang#grill-forest): split out of
// vite.config.ts because the tooling and the docs site sharing one dev server meant a docs-page
// edit and a tooling-code edit could each trigger a reload landing in the other app's tab, and
// Vite's full-reload fallback (forced when a module can't hot-swap) could wipe whatever the
// maintainer was mid-typing in the tooling app. `pnpm dev:tools` runs this instead of `pnpm dev`.

/** `dev/index.html`'s own script/favicon tags are absolute paths (`/dev/src/main.tsx`) written
 *  for the shared config's `root` (`site/`) -- the docs dev server's stale-URL fallback still
 *  serves this same file under that same root, so `root` can't just move to `dev/` here without
 *  breaking that. A plain redirect gets the same "visiting this server's bare URL reaches the
 *  tools app" result without touching either file's path assumptions. */
function redirectRootToDev(): Plugin {
  return {
    name: "redirect-root-to-dev",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        if (req.url === "/") {
          res.writeHead(302, { Location: "/dev/" })
          res.end()
          return
        }
        next()
      })
    },
  }
}

export default defineConfig({
  // No `base: "/toylang/"` here -- that prefix exists only because the docs site deploys under
  // kantord.github.io/toylang/. This app is dev-only and never deployed, so the default root
  // base is correct.
  plugins: [redirectRootToDev(), react(), tailwindcss(), annotationsInbox(), grillRounds(), grillForest()],
  resolve: {
    alias: { "@": import.meta.dirname + "/src", "@dev": import.meta.dirname + "/dev/src" },
  },
  server: {
    fs: { allow: [import.meta.dirname + "/.."] },
    // Must stay in sync with TOOLS_PORT in dev/src/DevApp.tsx, which warns when this app is
    // opened under the wrong dev server (e.g. `pnpm dev`'s default port) instead of this one.
    // Deliberately not 5174: that's exactly where Vite's own auto-increment lands the docs
    // server (`pnpm dev`, default 5173) if 5173 is already taken, which would collide with this
    // config the moment both happen to be starting up in that order.
    port: 5180,
    // No HMR at all, not just no full-reload fallback: a code change to this app needs a manual
    // refresh. The tradeoff (losing Vite's error overlay, which rides the same client) is
    // accepted -- the actual risk this guards against, losing in-progress typed input to a
    // forced reload, is independently covered by dev/src/lib/draft.ts's localStorage safety net.
    hmr: false,
  },
})
