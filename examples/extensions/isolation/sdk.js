// Client minimal du canal hôte (le vrai sera dans @island/sdk v2).
export function connect() {
  return new Promise((resolve) => {
    let done = false;
    addEventListener("message", (e) => {
      if (done || e.data?.t !== "island:connect" || !e.ports?.[0]) return;
      done = true;
      const port = e.ports[0];
      let n = 0;
      const pending = new Map();
      const listeners = new Map();
      port.onmessage = ({ data }) => {
        if (data?.t === "ret") {
          const p = pending.get(data.n);
          pending.delete(data.n);
          if (p) data.ok ? p.resolve(data.value) : p.reject(new Error(data.error));
        } else if (data?.t === "event") {
          for (const cb of listeners.get(data.name) ?? []) cb(data.data);
        }
      };
      const call = (method, ...args) =>
        new Promise((res, rej) => {
          const id = ++n;
          pending.set(id, { resolve: res, reject: rej });
          port.postMessage({ t: "call", n: id, method, args });
        });
      const on = (name, cb) => listeners.set(name, [...(listeners.get(name) ?? []), cb]);
      resolve({ call, on, info: e.data });
    });
  });
}
