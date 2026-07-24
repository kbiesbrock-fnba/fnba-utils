/**
 * Open a URL in the user's default browser. Required for Jira links because
 * WebView2 silently ignores `<a target="_blank">` without explicit shell-plugin
 * handling.
 */
export async function openExternal(url: string): Promise<void> {
  const { open } = await import("@tauri-apps/plugin-shell");
  await open(url);
}
