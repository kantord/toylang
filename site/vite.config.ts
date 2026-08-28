import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

import { annotationsInbox } from "./vite-plugins/annotations-inbox.ts"
import { grillRounds } from "./vite-plugins/grill-rounds.ts"

export default defineConfig({
  // The site is published at kantord.github.io/toylang/, so assets resolve under the repo name
  // rather than the domain root. `pnpm dev` overrides nothing: Vite serves this prefix locally
  // too, which is what stops a path working in development and 404ing once deployed.
  base: "/toylang/",
  plugins: [react(), tailwindcss(), annotationsInbox(), grillRounds()],
  resolve: {
    alias: { "@": import.meta.dirname + "/src" },
  },
  // The docs pages live at the repository root, beside the code and the harness that runs
  // their fragments, so the dev server must be allowed to read one level up.
  server: {
    fs: { allow: [import.meta.dirname + "/.."] },
  },
})
