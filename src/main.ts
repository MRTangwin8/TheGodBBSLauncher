import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import router from "./router";
import "./themes/mcbbs-classic.css";
import "./themes/palettes.css";
import "./themes/layouts.css";
import "./themes/buttons.css";
import { installShortcutGuard } from "./shortcuts";
import { setupPlatform } from "./platform";
import { i18n } from "./i18n";

installShortcutGuard();
setupPlatform();

createApp(App).use(createPinia()).use(i18n).use(router).mount("#app");
