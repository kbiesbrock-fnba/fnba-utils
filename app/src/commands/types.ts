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
  component: Component;
  breadcrumbs?: BreadcrumbStep[];
}
