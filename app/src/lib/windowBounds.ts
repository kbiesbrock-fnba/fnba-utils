// Guards persisted window geometry against display-topology changes.
//
// Viewer windows persist their last position/size (logical px) and restore it
// on the next launch. After a monitor is removed or a laptop is undocked, a
// rect saved on the now-detached display would reopen off-screen and
// unreachable — so callers validate the saved rect against the attached
// monitors first and drop the position if it no longer lands anywhere visible.

/**
 * Whether a persisted window rect (LOGICAL px) still overlaps an attached
 * monitor. Monitors report PHYSICAL px plus a scale factor, so each is
 * converted to logical before comparing; a small overlap on both axes is
 * required to count as visible. Fails open (returns true) if the monitor list
 * can't be read, so a transient query failure never discards a good position.
 */
export async function rectVisibleOnAnyMonitor(rect: {
  x: number;
  y: number;
  width: number;
  height: number;
}): Promise<boolean> {
  try {
    const { availableMonitors } = await import("@tauri-apps/api/window");
    const monitors = await availableMonitors();
    if (!monitors || monitors.length === 0) return true;

    const MIN_VISIBLE = 48; // logical px of overlap required on each axis
    for (const m of monitors) {
      const sf = m.scaleFactor || 1;
      const mx = m.position.x / sf;
      const my = m.position.y / sf;
      const mw = m.size.width / sf;
      const mh = m.size.height / sf;
      const overlapX = Math.min(rect.x + rect.width, mx + mw) - Math.max(rect.x, mx);
      const overlapY = Math.min(rect.y + rect.height, my + mh) - Math.max(rect.y, my);
      if (overlapX >= MIN_VISIBLE && overlapY >= MIN_VISIBLE) return true;
    }
    return false;
  } catch {
    return true;
  }
}
