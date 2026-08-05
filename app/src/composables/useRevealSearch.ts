// Tiny hidden-by-default/reveal-on-demand toggle for a search bar (Ctrl+F).
// Shared by JSON Viewer and Markdown Viewer.
import { ref } from "vue";

export function useRevealSearch() {
  const isOpen = ref(false);
  return {
    isOpen,
    open: () => (isOpen.value = true),
    close: () => (isOpen.value = false),
    toggle: () => (isOpen.value = !isOpen.value),
  };
}
