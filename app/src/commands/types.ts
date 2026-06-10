import type { Component } from "vue";

export interface BreadcrumbStep {
  label: string;
  steps: string[]; // internal step IDs this breadcrumb entry represents
}

export interface PaletteCommand {
  id: string;
  name: string;
  description: string;
  icon: string;
  keywords: string[];
  /** Component rendered when the command is selected. Omit for `action` (soft) commands. */
  component?: Component;
  /**
   * One-shot handler. When present, selecting the command runs this and
   * dismisses the palette instead of opening a `component`. Used by contextual
   * "soft commands" (see `lib/softCommands.ts`).
   */
  action?: () => void | Promise<void>;
  /**
   * Marks a contextual soft command — surfaced only when the typed query
   * matches a pattern (URL, JSON, Jira key, …) and no real command matches.
   */
  soft?: boolean;
  breadcrumbs?: BreadcrumbStep[];
}
