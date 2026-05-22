import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { resolve } from "path";
import { execFileSync } from "node:child_process";
import pkg from "./package.json";

const host = process.env.TAURI_DEV_HOST;

function gitCount(): string {
  try {
    return execFileSync("git", ["rev-list", "--count", "HEAD"], {
      stdio: ["ignore", "pipe", "ignore"],
    })
      .toString()
      .trim();
  } catch {
    return "0";
  }
}

const appVersion = `${pkg.version}+${gitCount()}`;

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      "@": resolve(__dirname, "src"),
    },
  },
  define: {
    __APP_VERSION__: JSON.stringify(appVersion),
  },
  build: {
    rollupOptions: {
      output: {
        // Lift heavy vendors into their own cacheable chunks so application
        // code splits stay well under Vite's 500 kB warning threshold.
        manualChunks(id) {
          if (id.includes("node_modules/@xterm/")) return "xterm";
          if (id.includes("node_modules/@tauri-apps/")) return "tauri";
          if (
            id.includes("node_modules/vue/") ||
            id.includes("node_modules/@vue/")
          ) {
            return "vue";
          }
        },
      },
    },
  },
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: host || "0.0.0.0",
    hmr: host ? { protocol: "ws", host, port: 5174 } : undefined,
    watch: {
      ignored: ["**/src-tauri/target/**"],
      usePolling: true,
    },
  },
});
