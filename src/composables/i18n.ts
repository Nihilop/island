// Helper i18n de l'HÔTE : `t()` lié au namespace "host". Réactif (re-render au switch
// de langue). La locale est pilotée par appStore.lang (cf. main.ts). Les extensions, elles,
// passent par `ctx.i18n.t` (namespacé par leur id).
import { translate, locale, setLocale } from "../sdk/i18n";

export const t = (key: string, params?: Record<string, unknown>) => translate("host", key, params);
export { locale, setLocale };
