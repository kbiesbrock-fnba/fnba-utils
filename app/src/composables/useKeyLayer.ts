import { onMounted, onUnmounted } from "vue";

export type KeyHandler = (e: KeyboardEvent) => boolean | void;

export interface KeyBinding {
  key: string;
  handler: KeyHandler;
  preventDefault?: boolean;
}

interface Layer {
  id: symbol;
  priority: number;
  bindings: Map<string, KeyBinding>;
}

const layers: Layer[] = [];
let listening = false;

function dispatch(e: KeyboardEvent) {
  for (const layer of layers) {
    const binding = layer.bindings.get(e.key);
    if (!binding) continue;

    if (binding.preventDefault !== false) {
      e.preventDefault();
    }

    const result = binding.handler(e);

    if (result !== false) {
      e.stopImmediatePropagation();
      return;
    }
  }
}

function ensureListener() {
  if (!listening) {
    window.addEventListener("keydown", dispatch, true);
    listening = true;
  }
}

function maybeRemoveListener() {
  if (layers.length === 0 && listening) {
    window.removeEventListener("keydown", dispatch, true);
    listening = false;
  }
}

function insertLayer(layer: Layer) {
  let i = 0;
  while (i < layers.length && layers[i].priority >= layer.priority) {
    i++;
  }
  layers.splice(i, 0, layer);
}

function removeLayer(id: symbol) {
  const idx = layers.findIndex((l) => l.id === id);
  if (idx >= 0) layers.splice(idx, 1);
}

export const KEY_PRIORITY = {
  PICKER: 300,
  COMMAND: 200,
  PALETTE: 100,
} as const;

export function useKeyLayer(
  bindings: KeyBinding[],
  options?: { priority?: number },
) {
  const id = Symbol();
  const priority = options?.priority ?? KEY_PRIORITY.PICKER;

  const layer: Layer = {
    id,
    priority,
    bindings: new Map(bindings.map((b) => [b.key, b])),
  };

  onMounted(() => {
    ensureListener();
    insertLayer(layer);
  });

  onUnmounted(() => {
    removeLayer(id);
    maybeRemoveListener();
  });
}
