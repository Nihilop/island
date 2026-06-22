import { defineConfig } from "vitepress";

// Doc Island — bilingue (FR par défaut, EN sous /en/). Déployée sur GitHub Pages.
// Le thème « island interactif » viendra dans docs/.vitepress/theme (phase 2).
export default defineConfig({
  title: "Island",
  description: "La Dynamic Island pour Windows — slots agnostiques + extensions (Vue 3, Tauri 2).",
  base: "/island/", // GitHub Pages projet : nihilop.github.io/island/
  cleanUrls: true,
  ignoreDeadLinks: true, // liens internes inter-fichiers à repasser après la réorg
  lastUpdated: true,

  themeConfig: {
    search: { provider: "local" },
    socialLinks: [{ icon: "github", link: "https://github.com/Nihilop/island" }],
  },

  locales: {
    root: {
      label: "Français",
      lang: "fr",
      themeConfig: {
        nav: [
          { text: "Guide", link: "/guide/introduction" },
          { text: "Référence", link: "/reference/authoring" },
          { text: "Extensions", link: "/extensions/toolchain" },
        ],
        sidebar: [
          {
            text: "Guide",
            items: [
              { text: "Introduction", link: "/guide/introduction" },
              { text: "Démarrage rapide", link: "/guide/demarrage" },
              { text: "Anatomie & build", link: "/guide/anatomie" },
              { text: "Le SDK", link: "/guide/sdk" },
              { text: "Services & permissions", link: "/guide/services" },
              { text: "Recettes", link: "/guide/recettes" },
              { text: "Distribution", link: "/guide/distribution" },
            ],
          },
          {
            text: "Référence",
            items: [
              { text: "Écrire une extension", link: "/reference/authoring" },
              { text: "Catalogue des services", link: "/reference/services-catalog" },
              { text: "Intégrations & serve", link: "/reference/integrations" },
              { text: "Mises à jour & releases", link: "/reference/updates" },
              { text: "Roadmap SDK", link: "/reference/roadmap" },
              { text: "Perf : gel de l'overlay", link: "/reference/perf-overlay-freeze" },
            ],
          },
          {
            text: "Extensions",
            items: [
              { text: "Toolchain", link: "/extensions/toolchain" },
              { text: "Docker", link: "/extensions/docker" },
            ],
          },
        ],
        docFooter: { prev: "Précédent", next: "Suivant" },
        outline: { label: "Sur cette page" },
        lastUpdatedText: "Mis à jour le",
      },
    },

    en: {
      label: "English",
      lang: "en",
      link: "/en/",
      themeConfig: {
        nav: [
          { text: "Guide", link: "/en/guide/introduction" },
          { text: "Reference", link: "/en/reference/authoring" },
          { text: "Extensions", link: "/en/extensions/toolchain" },
        ],
        sidebar: [
          {
            text: "Guide",
            items: [
              { text: "Introduction", link: "/en/guide/introduction" },
              { text: "Quick start", link: "/en/guide/demarrage" },
              { text: "Anatomy & build", link: "/en/guide/anatomie" },
              { text: "The SDK", link: "/en/guide/sdk" },
              { text: "Services & permissions", link: "/en/guide/services" },
              { text: "Recipes", link: "/en/guide/recettes" },
              { text: "Distribution", link: "/en/guide/distribution" },
            ],
          },
          {
            text: "Reference",
            items: [
              { text: "Authoring an extension", link: "/en/reference/authoring" },
              { text: "Services catalog", link: "/en/reference/services-catalog" },
              { text: "Integrations & serve", link: "/en/reference/integrations" },
              { text: "Updates & releases", link: "/en/reference/updates" },
              { text: "SDK roadmap", link: "/en/reference/roadmap" },
              { text: "Perf: overlay freeze", link: "/en/reference/perf-overlay-freeze" },
            ],
          },
          {
            text: "Extensions",
            items: [
              { text: "Toolchain", link: "/en/extensions/toolchain" },
              { text: "Docker", link: "/en/extensions/docker" },
            ],
          },
        ],
        docFooter: { prev: "Previous", next: "Next" },
        outline: { label: "On this page" },
        lastUpdatedText: "Updated on",
      },
    },
  },
});
