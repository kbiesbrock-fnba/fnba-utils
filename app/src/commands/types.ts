import type { Component } from "vue";

export interface PaletteCommand {
  id: string;
  name: string;
  description: string;
  icon: string;
  keywords: string[];
  component: Component;
}
