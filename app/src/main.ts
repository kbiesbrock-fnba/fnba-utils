import { createApp } from "vue";
import App from "./App.vue";
import "./style.css";
import { initCommands } from "./commands";

// Fire-and-forget — don't block mount on detecting opt-in features.
void initCommands();

createApp(App).mount("#app");
