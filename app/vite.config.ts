import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { resolve } from "path";
import { computeVersion } from "./scripts/version.mjs";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      "@": resolve(__dirname, "src"),
    },
  },
  define: {
    __APP_VERSION__: JSON.stringify(computeVersion()),
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
