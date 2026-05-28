import { ref, watch, nextTick, type Ref } from "vue";
import { useKeyLayer, KEY_PRIORITY, type KeyBinding } from "./useKeyLayer";

interface ListNavigationOptions {
  itemCount: Ref<number> | (() => number);
  onSelect?: (index: number) => void;
  onEnterEmpty?: () => void;
  extraKeys?: KeyBinding[];
  listRef?: Ref<HTMLElement | null>;
  scrollStrategy?: "data-index" | "children" | "selected-class";
  priority?: number;
}

export function useListNavigation(options: ListNavigationOptions) {
  const selectedIndex = ref(0);

  function getCount(): number {
    const c = options.itemCount;
    return typeof c === "function" ? c() : c.value;
  }

  const bindings: KeyBinding[] = [
    {
      key: "ArrowDown",
      handler: () => {
        const count = getCount();
        if (count > 0) {
          selectedIndex.value = (selectedIndex.value + 1) % count;
        }
      },
    },
    {
      key: "ArrowUp",
      handler: () => {
        const count = getCount();
        if (count > 0) {
          selectedIndex.value = (selectedIndex.value - 1 + count) % count;
        }
      },
    },
  ];

  if (options.onSelect || options.onEnterEmpty) {
    bindings.push({
      key: "Enter",
      handler: (e) => {
        if (e.repeat) return;
        const count = getCount();
        if (count > 0 && options.onSelect) {
          options.onSelect(selectedIndex.value);
        } else if (count === 0 && options.onEnterEmpty) {
          options.onEnterEmpty();
        }
      },
    });
  }

  if (options.extraKeys) {
    for (const extra of options.extraKeys) {
      bindings.push(extra);
    }
  }

  useKeyLayer(bindings, { priority: options.priority ?? KEY_PRIORITY.PICKER });

  if (options.listRef) {
    const listRef = options.listRef;
    const strategy = options.scrollStrategy ?? "children";

    watch(selectedIndex, () => {
      nextTick(() => {
        const list = listRef.value;
        if (!list) return;

        let el: HTMLElement | null = null;
        if (strategy === "data-index") {
          el = list.querySelector(`[data-index="${selectedIndex.value}"]`);
        } else if (strategy === "selected-class") {
          el = list.querySelector(".selected");
        } else {
          el = list.children[selectedIndex.value] as HTMLElement | null;
        }
        el?.scrollIntoView({ block: "nearest" });
      });
    });
  }

  function resetIndex() {
    selectedIndex.value = 0;
  }

  return { selectedIndex, resetIndex };
}
