import { connect } from "./sdk.js";
import { selftest } from "./selftest.js";

const api = await connect();
await api.call("log", "logique démarrée");
const results = await selftest(api);
const failed = results.filter((x) => !x.pass);
await api.call("log", failed.length ? `ÉCHEC isolation : ${failed.map((f) => `${f.name} (${f.detail})`).join(" ; ")}` : `isolation OK (${results.length} vérifications)`);
