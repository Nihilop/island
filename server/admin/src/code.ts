// Lecture du code d'une extension à relire : mise en forme (js-beautify, utile pour le code
// minifié) puis coloration (highlight.js). Chargés à la demande : le reste du panneau n'en
// a pas besoin. `hljs` échappe le texte, son HTML est donc sûr à injecter.

type Lang = "javascript" | "css" | "xml" | "json" | "markdown" | "plaintext";

export function langOf(path: string): Lang {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  if (["js", "mjs", "cjs", "ts"].includes(ext)) return "javascript";
  if (ext === "css") return "css";
  if (["html", "htm", "svg", "xml"].includes(ext)) return "xml";
  if (["json", "map"].includes(ext)) return "json";
  if (ext === "md") return "markdown";
  return "plaintext";
}

/** Code minifié : des lignes très longues. */
export function looksMinified(text: string): boolean {
  const lines = text.split("\n");
  return lines.some((l) => l.length > 300) || text.length / Math.max(lines.length, 1) > 160;
}

type Beautify = typeof import("js-beautify");
type Libs = { hljs: typeof import("highlight.js/lib/core").default; beautify: Beautify };
let libs: Promise<Libs> | null = null;

async function importAll(): Promise<Libs> {
  const [{ default: hljs }, js, css, xml, json, md, mod] = await Promise.all([
    import("highlight.js/lib/core"),
    import("highlight.js/lib/languages/javascript"),
    import("highlight.js/lib/languages/css"),
    import("highlight.js/lib/languages/xml"),
    import("highlight.js/lib/languages/json"),
    import("highlight.js/lib/languages/markdown"),
    import("js-beautify"),
  ]);
  hljs.registerLanguage("javascript", js.default);
  hljs.registerLanguage("css", css.default);
  hljs.registerLanguage("xml", xml.default);
  hljs.registerLanguage("json", json.default);
  hljs.registerLanguage("markdown", md.default);
  // js-beautify est en CommonJS : selon le bundler, l'API est sur `default` ou à la racine.
  const beautify = ((mod as { default?: Beautify }).default ?? mod) as Beautify;
  return { hljs, beautify };
}

const load = (): Promise<Libs> => (libs ??= importAll());

/** HTML coloré (et mis en forme si `format`). */
export async function render(text: string, path: string, format: boolean): Promise<string> {
  const { hljs, beautify } = await load();
  const lang = langOf(path);
  let src = text;
  if (format) {
    const opts = { indent_size: 2, wrap_line_length: 120 };
    if (lang === "javascript" || lang === "json") src = beautify.js(text, opts);
    else if (lang === "css") src = beautify.css(text, opts);
    else if (lang === "xml") src = beautify.html(text, opts);
  }
  if (lang === "plaintext") return hljs.highlight(src, { language: "markdown", ignoreIllegals: true }).value;
  return hljs.highlight(src, { language: lang, ignoreIllegals: true }).value;
}
