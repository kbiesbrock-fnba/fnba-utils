import { isTauri } from "@/lib/tauri";

/**
 * Thin wrapper around `tauri-plugin-notification`. Used by Wave 4 features
 * (permission-prompt detection, busy→idle transitions) to surface a system
 * toast when the relevant window is unfocused. Browser-dev mode logs to
 * console.
 *
 * Callers should gate on focus themselves — this just fires.
 */

let permissionChecked = false;
let permissionGranted = false;

async function ensurePermission(): Promise<boolean> {
  if (permissionChecked) return permissionGranted;
  permissionChecked = true;
  if (!isTauri) {
    permissionGranted = true;
    return true;
  }
  try {
    const { isPermissionGranted, requestPermission } = await import(
      "@tauri-apps/plugin-notification"
    );
    if (await isPermissionGranted()) {
      permissionGranted = true;
    } else {
      const status = await requestPermission();
      permissionGranted = status === "granted";
    }
  } catch (e) {
    console.warn("[notifications] permission check failed", e);
    permissionGranted = false;
  }
  return permissionGranted;
}

export interface ToastOptions {
  title: string;
  body?: string;
}

export async function notify(opts: ToastOptions): Promise<void> {
  if (!(await ensurePermission())) {
    console.log("[notify] permission denied", opts);
    return;
  }
  if (!isTauri) {
    console.log("[notify]", opts);
    return;
  }
  try {
    const { sendNotification } = await import("@tauri-apps/plugin-notification");
    sendNotification({ title: opts.title, body: opts.body });
  } catch (e) {
    console.warn("[notifications] sendNotification failed", e);
  }
}

/**
 * Is any window in the MC window group currently focused? Used to suppress
 * notifications when the user is already looking at the relevant panel.
 *
 * "Focused" here means the OS-level focus is on the main palette, mission
 * control, or any session-detail panel. SQL-query panels also count since
 * they're sibling tools.
 */
export async function isAnyMcWindowFocused(): Promise<boolean> {
  if (!isTauri) return true; // dev: avoid spam
  try {
    const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
    const all = await WebviewWindow.getAll();
    const focused = await Promise.all(all.map((w) => w.isFocused()));
    return focused.some(Boolean);
  } catch {
    return false;
  }
}
