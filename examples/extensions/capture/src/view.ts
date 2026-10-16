// Vue de l'outil Capture (dans l'île).
import "@island/sdk/ui.css";
import { createApp } from "vue";
import { connect } from "@island/sdk";
import App from "./App.vue";

const island = await connect();
createApp(App, { island }).mount("#app");
