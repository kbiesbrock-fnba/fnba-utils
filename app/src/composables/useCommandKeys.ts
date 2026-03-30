import { type Ref } from "vue";
import { useKeyLayer, KEY_PRIORITY } from "./useKeyLayer";

interface CommandKeysOptions<S extends string> {
  step: Ref<S>;
  goBack: () => boolean;
  emitBack: () => void;
  emitDismiss: () => void;
  enterActions?: Partial<Record<S, () => void>>;
  escapeDismissSteps?: S[];
}

export function useCommandKeys<S extends string>(options: CommandKeysOptions<S>) {
  const { step, goBack, emitBack, emitDismiss } = options;
  const escapeDismissSteps = new Set(options.escapeDismissSteps ?? []);
  const enterActions: Partial<Record<S, () => void>> = options.enterActions ?? {};

  useKeyLayer(
    [
      {
        key: "Escape",
        handler: () => {
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
