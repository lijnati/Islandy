/// <reference types="vitest/config" />
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { defineConfig } from "vite";

const pkg = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8")) as {
  version: string;
};

// Set by `tauri dev` when targeting a physical device; unused on desktop.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react(), tailwindcss()],
  // Tauri expects a fixed port; fail instead of silently picking another one.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host ?? false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  define: { __APP_VERSION__: JSON.stringify(pkg.version) },
  build: {
    // WebView2 on Windows 10/11 is evergreen Chromium.
    target: "chrome110",
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: Boolean(process.env.TAURI_ENV_DEBUG),
    // Two pages: the island (index.html) and the settings window.
    rollupOptions: {
      input: {
        island: resolve(import.meta.dirname, "index.html"),
        settings: resolve(import.meta.dirname, "settings.html"),
      },
    },
  },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
    setupFiles: ["src/test/setup.ts"],
    restoreMocks: true,
  },
});
