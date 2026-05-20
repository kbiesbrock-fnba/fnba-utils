import { isTauri } from "./tauri";

/**
 * Open a URL in the user's default browser. Required for Jira links because
 * WebView2 silently ignores `<a target="_blank">` without explicit shell-plugin
 * handling. Falls back to `window.open()` in browser dev mode.
 */
export async function openExternal(url: string): Promise<void> {
  if (isTauri) {
    const { open } = await import("@tauri-apps/plugin-shell");
    await open(url);
    return;
  }
  window.open(url, "_blank", "noopener,noreferrer");
}
