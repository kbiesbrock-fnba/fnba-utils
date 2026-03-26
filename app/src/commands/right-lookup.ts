import type { PaletteCommand } from "./types";
import RightLookupCommand from "../components/right-lookup/RightLookupCommand.vue";

export const rightLookupCommand: PaletteCommand = {
  id: "right-lookup",
  name: "Right Lookup",
  description: "Find who has a specific right",
  icon: "\uD83D\uDD11",
  keywords: ["right", "rights", "permission", "group", "associate", "lookup", "notedb"],
  component: RightLookupCommand,
};
