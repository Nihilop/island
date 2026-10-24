// Vue « Projets » dans le notch.

import "@island/sdk/ui.css";
import "./style.css";
import { createApp } from "vue";
import { connect } from "@island/sdk";
import App from "./App.vue";

const island = await connect();
createApp(App, { island }).mount("#app");
