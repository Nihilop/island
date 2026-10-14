import { connect } from "./sdk.js";
import { selftest } from "./selftest.js";

const api = await connect();
const applyTheme = (t) => document.documentElement.classList.toggle("light", t === "light");
applyTheme(api.info.theme);
api.on("theme", applyTheme);

const results = await selftest(api);
const ok = results.filter((x) => x.pass).length;
document.getElementById("sum").textContent = `${ok} / ${results.length} vérifications réussies (vue d'outil)`;
document.getElementById("list").innerHTML = results
  .map((x) => `<li class="${x.pass ? "ok" : "ko"}"><b>${x.pass ? "✓" : "✗"}</b><span>${x.name}</span><span>${x.detail.replace(/</g, "&lt;")}</span></li>`)
  .join("");
