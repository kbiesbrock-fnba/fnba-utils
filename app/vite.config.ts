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
