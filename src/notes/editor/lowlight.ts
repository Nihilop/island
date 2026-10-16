// Coloration syntaxique des blocs de code : lowlight (highlight.js en AST) limité aux langages
// courants, pour ne pas embarquer les ~190 grammaires de highlight.js.
import { createLowlight } from "lowlight";
import bash from "highlight.js/lib/languages/bash";
import c from "highlight.js/lib/languages/c";
import cpp from "highlight.js/lib/languages/cpp";
import csharp from "highlight.js/lib/languages/csharp";
import css from "highlight.js/lib/languages/css";
import diff from "highlight.js/lib/languages/diff";
import go from "highlight.js/lib/languages/go";
import ini from "highlight.js/lib/languages/ini";
import java from "highlight.js/lib/languages/java";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import markdown from "highlight.js/lib/languages/markdown";
import php from "highlight.js/lib/languages/php";
import powershell from "highlight.js/lib/languages/powershell";
import python from "highlight.js/lib/languages/python";
import rust from "highlight.js/lib/languages/rust";
import sql from "highlight.js/lib/languages/sql";
import typescript from "highlight.js/lib/languages/typescript";
import xml from "highlight.js/lib/languages/xml";
import yaml from "highlight.js/lib/languages/yaml";

export const lowlight = createLowlight({ bash, c, cpp, csharp, css, diff, go, ini, java, javascript, json, markdown, php, powershell, python, rust, sql, typescript, xml, yaml });

// Alias saisis après ``` (les grammaires en déclarent déjà une partie : js, ts, sh, py, rs, yml, md…).
lowlight.registerAlias({ xml: ["html", "svg", "vue"], bash: ["shell", "zsh", "console"], json: ["jsonc"], ini: ["toml"] });

/** Langages proposés dans le sélecteur des blocs de code (id = nom écrit après ```). */
export const LANGUAGES = [
  { id: "bash", label: "Bash" },
  { id: "c", label: "C" },
  { id: "cpp", label: "C++" },
  { id: "csharp", label: "C#" },
  { id: "css", label: "CSS" },
  { id: "diff", label: "Diff" },
  { id: "go", label: "Go" },
  { id: "html", label: "HTML" },
  { id: "java", label: "Java" },
  { id: "javascript", label: "JavaScript" },
  { id: "json", label: "JSON" },
  { id: "markdown", label: "Markdown" },
  { id: "php", label: "PHP" },
  { id: "powershell", label: "PowerShell" },
  { id: "python", label: "Python" },
  { id: "rust", label: "Rust" },
  { id: "sql", label: "SQL" },
  { id: "toml", label: "TOML" },
  { id: "typescript", label: "TypeScript" },
  { id: "vue", label: "Vue" },
  { id: "xml", label: "XML" },
  { id: "yaml", label: "YAML" },
];
