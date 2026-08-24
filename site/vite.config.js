import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
export default defineConfig({
    // The site is published at kantord.github.io/toylang/, so assets resolve under the repo name
    // rather than the domain root. `pnpm dev` overrides nothing: Vite serves this prefix locally
    // too, which is what stops a path working in development and 404ing once deployed.
    base: "/toylang/",
    plugins: [react(), tailwindcss()],
    resolve: {
        alias: { "@": path.resolve(__dirname, "./src") },
    },
});
