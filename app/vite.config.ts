import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import svgr from "vite-plugin-svgr";
import { defineConfig } from "vitest/config";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react(), tailwindcss(), svgr()],
  clearScreen: false,
  test: {
    coverage: {
      provider: "v8",
      reporter: ["text", "html"],
      reportsDirectory: "coverage",
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "src/**/*.d.ts",
        "src/vite-env.d.ts",
        "src/main.tsx",
        "src/overlay-main.tsx",
        "src/overlay-hover-main.tsx",
        "src/quick-ask-main.tsx",
      ],
    },
  },
  server: {
    host: host || false,
    port: 5173,
    strictPort: true,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 5173,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target:
      process.env.TAURI_PLATFORM === "windows"
        ? "chrome105"
        : process.env.TAURI_PLATFORM === "macos"
          ? "safari13"
          : "chrome105",
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
    rollupOptions: {
      input: {
        main: "index.html",
        overlay: "overlay.html",
        overlayHover: "overlay-hover.html",
        quickAsk: "quick-ask.html",
      },
    },
  },
});
