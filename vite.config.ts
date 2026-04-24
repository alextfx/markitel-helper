import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed dev port so its window can attach; 5174 keeps it
// out of the Next.js app's usual 5173.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5174,
    strictPort: true,
  },
  // Disable PostCSS autoload. Without this Vite walks up the directory
  // tree and picks up the repo-root `postcss.config.js`, which pulls in
  // tailwindcss — installed at the root for the Next.js app, but NOT in
  // helper-app/node_modules. In CI that makes the build crash with
  // "Cannot find module 'tailwindcss'". The helper UI doesn't use
  // PostCSS anyway.
  css: { postcss: { plugins: [] } },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    target: "esnext",
  },
});
