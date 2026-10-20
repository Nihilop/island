import { defineConfig } from "vitepress";

// Doc Island — bilingue (FR par défaut, EN sous /en/). Déployée sur GitHub Pages.
// Le thème « island interactif » viendra dans docs/.vitepress/theme (phase 2).
export default defineConfig({
  title: "Island",
  description: "Island, le notch de Windows — et ses extensions (iframes isolées, paquets signés).",
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
          { text: "Mises à jour", link: "/reference/updates" },
        ],
        sidebar: [
          {
            text: "Écrire une extension",
            items: [
              { text: "Introduction", link: "/guide/introduction" },
              { text: "Démarrage rapide", link: "/guide/demarrage" },
              { text: "Manifeste & build", link: "/guide/manifeste" },
              { text: "Le SDK", link: "/guide/sdk" },
              { text: "Emplacements", link: "/guide/emplacements" },
              { text: "Permissions & sécurité", link: "/guide/permissions" },
              { text: "Recettes", link: "/guide/recettes" },
              { text: "Distribution", link: "/guide/distribution" },
            ],
          },
          {
            text: "Island",
            items: [
              { text: "Mises à jour & releases", link: "/reference/updates" },
              { text: "Le magasin", link: "/reference/magasin" },
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
          { text: "Updates", link: "/en/reference/updates" },
        ],
        sidebar: [
          {
            text: "Writing an extension",
            items: [
              { text: "Introduction", link: "/en/guide/introduction" },
              { text: "Quick start", link: "/en/guide/demarrage" },
              { text: "Manifest & build", link: "/en/guide/manifeste" },
              { text: "The SDK", link: "/en/guide/sdk" },
              { text: "Slots", link: "/en/guide/emplacements" },
              { text: "Permissions & security", link: "/en/guide/permissions" },
              { text: "Recipes", link: "/en/guide/recettes" },
              { text: "Distribution", link: "/en/guide/distribution" },
            ],
          },
          {
            text: "Island",
            items: [
              { text: "Updates & releases", link: "/en/reference/updates" },
              { text: "The store", link: "/en/reference/magasin" },
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
