import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import {
  clearClipboardHistory,
  deleteClipboardEntry,
  getClipboardEntry,
  getClipboardMaxCapturedAt,
  hideClipboardWindow,
  listClipboardEntries,
  onClipboardEntryUpdated,
  onClipboardWindowShown,
  pasteClipboardEntry,
  pinClipboardEntry,
  requestSensitiveReveal,
  setClipboardEntryLabel,
  setClipboardEntrySensitivity,
  updateClipboardEntryContent,
  type ClipboardEntryFull,
  type ClipboardEntrySummary,
  type ClipboardKind,
} from "@/lib/tauri";

export type Filter = "all" | ClipboardKind | "pinned";

export function useClipboardManager() {
  const entries = ref<ClipboardEntrySummary[]>([]);
  const selectedId = ref<number | null>(null);
  const detail = ref<ClipboardEntryFull | null>(null);
  const detailLoading = ref(false);
  const query = ref("");
  const filter = ref<Filter>("all");
  const loading = ref(false);
  const error = ref<string | null>(null);

  let debounceTimer: ReturnType<typeof setTimeout> | null = null;
  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let unsubShown: (() => void) | null = null;
  let unsubUpdated: (() => void) | null = null;
  let lastSeenCapturedAt = 0;
  const POLL_INTERVAL_MS = 1500;

  const selected = computed(() =>
    entries.value.find((e) => e.id === selectedId.value) ?? null,
  );

  async function load() {
    loading.value = true;
    try {
      const kind = filter.value === "all" || filter.value === "pinned"
        ? undefined
        : (filter.value as ClipboardKind);
      const rows = await listClipboardEntries(
        query.value || undefined,
        kind,
        filter.value === "pinned",
        200,
        0,
      );
      entries.value = rows;
      // Keep the selection if still present; otherwise pick the first row.
      if (
        selectedId.value == null ||
        !rows.some((r) => r.id === selectedId.value)
      ) {
        selectedId.value = rows[0]?.id ?? null;
      }
      // Track the freshest captured_at so polling can short-circuit when
      // nothing new has landed.
      lastSeenCapturedAt = rows.reduce(
        (m, r) => (r.capturedAt > m ? r.capturedAt : m),
        lastSeenCapturedAt,
      );
      error.value = null;
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
    } finally {
      loading.value = false;
    }
  }

  async function pollForNew() {
    try {
      const latest = await getClipboardMaxCapturedAt();
      if (latest > lastSeenCapturedAt) {
        // New capture from the daemon — reload to surface it.
        await load();
      }
    } catch {
      // Polling is best-effort; ignore transient errors (e.g. DB locked).
    }
  }

  function scheduleReload() {
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => void load(), 120);
  }

  watch([query, filter], () => scheduleReload());

  watch(selectedId, async (id) => {
    if (id == null) {
      detail.value = null;
      return;
    }
    detailLoading.value = true;
    try {
      detail.value = await getClipboardEntry(id);
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
    } finally {
      detailLoading.value = false;
    }
  });

  function selectIndex(delta: number) {
    if (entries.value.length === 0) return;
    const currentIdx = entries.value.findIndex((e) => e.id === selectedId.value);
    const nextIdx = Math.max(
      0,
      Math.min(entries.value.length - 1, (currentIdx < 0 ? 0 : currentIdx) + delta),
    );
    selectedId.value = entries.value[nextIdx].id;
  }

  function selectFirst() {
    if (entries.value.length) selectedId.value = entries.value[0].id;
  }

  function selectLast() {
    if (entries.value.length) {
      selectedId.value = entries.value[entries.value.length - 1].id;
    }
  }

  async function paste(opts: { simulate: boolean; original: boolean }) {
    const entry = selected.value;
    if (!entry) return;
    try {
      // Only sensitive + paste_original needs the reveal-token round-trip.
      // The default (paste_original=false) writes the stored obfuscated text
      // directly — no token required.
      let revealToken: string | undefined;
      if (entry.sensitive && opts.original) {
        const t = await requestSensitiveReveal(entry.id);
        revealToken = t.token;
      }
      await pasteClipboardEntry(entry.id, {
        simulatePaste: opts.simulate,
        pasteOriginal: opts.original,
        revealToken,
      });
      await hideClipboardWindow();
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
    }
  }

  async function togglePin(id?: number) {
    const target = id ?? selectedId.value;
    if (target == null) return;
    const row = entries.value.find((r) => r.id === target);
    if (!row) return;
    try {
      await pinClipboardEntry(target, !row.pinned);
      row.pinned = !row.pinned;
      // Reload to honor the pinned-first ordering.
      void load();
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
    }
  }

  async function remove(id?: number) {
    const target = id ?? selectedId.value;
    if (target == null) return;
    try {
      await deleteClipboardEntry(target);
      // Advance selection before removing the row so the UI doesn't blank.
      const idx = entries.value.findIndex((r) => r.id === target);
      entries.value = entries.value.filter((r) => r.id !== target);
      selectedId.value =
        entries.value[idx]?.id ?? entries.value[idx - 1]?.id ?? null;
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
    }
  }

  async function clearAll(includePinned: boolean) {
    try {
      await clearClipboardHistory(includePinned);
      await load();
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
    }
  }

  async function renameEntry(id: number, label: string | null) {
    try {
      await setClipboardEntryLabel(id, label);
      const trimmed = label?.trim();
      const next = trimmed ? trimmed : null;
      const row = entries.value.find((r) => r.id === id);
      if (row) row.label = next;
      if (detail.value?.id === id) detail.value = { ...detail.value, label: next };
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
    }
  }

  async function editEntryContent(id: number, content: string) {
    try {
      await updateClipboardEntryContent(id, content);
      // Refresh row + detail so the new content + recomputed byte size show up.
      // Skip the full list reload — the row stays in the same position.
      const fresh = await getClipboardEntry(id);
      if (fresh) {
        const idx = entries.value.findIndex((r) => r.id === id);
        if (idx >= 0) {
          entries.value[idx] = {
            ...entries.value[idx],
            kind: fresh.kind,
            textPreview: fresh.textContent?.slice(0, 240) ?? null,
            byteSize: fresh.byteSize,
            sensitive: fresh.sensitive,
            piiKinds: fresh.piiKinds,
            label: fresh.label,
          };
        }
        if (detail.value?.id === id) detail.value = fresh;
      }
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
      throw e;
    }
  }

  async function toggleSensitive(id?: number) {
    const target = id ?? selectedId.value;
    if (target == null) return;
    const row = entries.value.find((r) => r.id === target);
    if (!row) return;
    try {
      const next = !row.sensitive;
      await setClipboardEntrySensitivity(target, next);
      // Refetch the row + detail — the obfuscated text + pii_kinds change.
      const fresh = await getClipboardEntry(target);
      if (fresh) {
        const idx = entries.value.findIndex((r) => r.id === target);
        if (idx >= 0) {
          entries.value[idx] = {
            ...entries.value[idx],
            sensitive: fresh.sensitive,
            piiKinds: fresh.piiKinds,
            textPreview: fresh.sensitive
              ? fresh.obfuscatedText ?? fresh.textContent ?? null
              : fresh.textContent ?? null,
          };
        }
        if (detail.value?.id === target) detail.value = fresh;
      }
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
    }
  }

  function close() {
    void hideClipboardWindow();
  }

  onMounted(async () => {
    await load();
    // Capture lives in a separate daemon process now, so we can't subscribe
    // to in-process events for new entries. Instead, poll the DB for the
    // latest captured_at while the window is open and reload when it bumps.
    pollTimer = setInterval(() => void pollForNew(), POLL_INTERVAL_MS);
    unsubUpdated = await onClipboardEntryUpdated((id) => {
      // External mutation (or another component) updated this entry — refresh
      // only the affected row + detail. No full reload to avoid jitter.
      void (async () => {
        const fresh = await getClipboardEntry(id);
        if (!fresh) return;
        const idx = entries.value.findIndex((r) => r.id === id);
        if (idx >= 0) {
          entries.value[idx] = {
            ...entries.value[idx],
            kind: fresh.kind,
            textPreview: fresh.sensitive
              ? fresh.obfuscatedText ?? fresh.textContent ?? null
              : fresh.textContent ?? null,
            byteSize: fresh.byteSize,
            sensitive: fresh.sensitive,
            piiKinds: fresh.piiKinds,
            label: fresh.label,
          };
        }
        if (detail.value?.id === id) detail.value = fresh;
      })();
    });
    unsubShown = await onClipboardWindowShown((p) => {
      // When the window is reopened via the global shortcut, reset the
      // query so the user lands on the freshest entries. Win+Shift+V sends
      // initialFilter="pinned" to land directly on the pinned view.
      query.value = "";
      filter.value = p?.initialFilter === "pinned" ? "pinned" : "all";
      // Drop the prior selection so the reload selects the top (freshest)
      // entry. The window is only hidden — not destroyed — so its list keeps
      // the old scroll offset across opens; re-selecting the first row makes
      // the selection-scroll watch pull the list back to the top.
      selectedId.value = null;
      void load();
    });
  });

  onUnmounted(() => {
    if (debounceTimer) clearTimeout(debounceTimer);
    if (pollTimer) clearInterval(pollTimer);
    if (unsubShown) unsubShown();
    if (unsubUpdated) unsubUpdated();
  });

  return {
    entries,
    selectedId,
    selected,
    detail,
    detailLoading,
    query,
    filter,
    loading,
    error,
    load,
    selectIndex,
    selectFirst,
    selectLast,
    paste,
    togglePin,
    remove,
    clearAll,
    renameEntry,
    editEntryContent,
    toggleSensitive,
    close,
  };
}
