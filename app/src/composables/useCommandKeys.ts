import { type Ref } from "vue";
import { useKeyLayer, KEY_PRIORITY } from "./useKeyLayer";

interface CommandKeysOptions<S extends string> {
  step: Ref<S>;
  goBack: () => boolean;
  emitBack: () => void;
  emitDismiss: () => void;
  enterActions?: Partial<Record<S, () => void>>;
  escapeDismissSteps?: S[];
  /** Steps where Escape is a consumed no-op: the key is swallowed (never falls
   *  through to the palette layer) but does nothing — no back, no dismiss. */
  escapeNoopSteps?: S[];
}

export function useCommandKeys<S extends string>(options: CommandKeysOptions<S>) {
  const { step, goBack, emitBack, emitDismiss } = options;
  const escapeDismissSteps = new Set(options.escapeDismissSteps ?? []);
  const escapeNoopSteps = new Set(options.escapeNoopSteps ?? []);
  const enterActions: Partial<Record<S, () => void>> = options.enterActions ?? {};

  useKeyLayer(
    [
      {
        key: "Escape",
        handler: () => {
          // Swallow the key but do nothing (returns undefined → the dispatcher
          // stops propagation, so it can't reach the palette layer).
          if (escapeNoopSteps.has(step.value)) {
            return;
          }
          if (escapeDismissSteps.has(step.value)) {
            emitDismiss();
            return;
          }
          if (!goBack()) {
            emitBack();
          }
        },
      },
      {
        key: "Enter",
        handler: () => {
          const action = enterActions[step.value];
          if (action) {
            action();
            return;
          }
          return false;
        },
      },
    ],
    { priority: KEY_PRIORITY.COMMAND },
  );
}
