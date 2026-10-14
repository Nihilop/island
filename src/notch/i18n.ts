import { translate } from "../sdk/i18n";

/** Traduction du namespace hôte. */
export const t = (key: string, params?: Record<string, unknown>) => translate("host", key, params);
