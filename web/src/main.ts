import { createApp } from "vue";
import App from "./App.vue";
import { router } from "./app/router";
import "./design/tokens.css";
import "./design/base.css";

createApp(App).use(router).mount("#app");
