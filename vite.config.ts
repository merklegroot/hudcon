import { defineConfig } from "vite";

export default defineConfig({
  root: "ui",
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari14",
    minify: process.env.TAURI_ENV_DEBUG ? false : "esbuild",
  },
});
