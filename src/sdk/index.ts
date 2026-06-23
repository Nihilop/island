// @island/sdk — point d'entrée (livré avec Island).
export { defineExtension, useIsland } from "./island";
export type { ExtensionDef, ExtensionContext, IslandApi, MediaState, ModalRequest, IdleState, IdleAction, ExtStorage, ExtSecrets, LauncherResult, Display, CaptureOptions, Region, SysStats, WindowInfo, NotificationSpec } from "./island";
// i18n partagé (host + extensions). Les extensions passent par ctx.i18n ; ces exports
// servent surtout l'hôte (pilotage de la locale) et le typage.
export { locale, setLocale, registerMessages, translate } from "./i18n";
export type { MessageLoader, Dict } from "./i18n";

// Button = composant shadcn-vue (design system), re-exporté par le SDK.
export { Button, buttonVariants } from "./ui/button";
export { Switch } from "./ui/switch"
export { Progress } from "./ui/progress"
export { Kbd, KbdGroup } from "./ui/kbd"
export { 
    Select, 
    SelectContent,
    SelectGroup,
    SelectItem,
    SelectItemText,
    SelectLabel,
    SelectScrollDownButton,
    SelectScrollUpButton,
    SelectSeparator,
    SelectTrigger,
    SelectValue
} from "./ui/select"

export {
    ContextMenu,
    ContextMenuTrigger,
    ContextMenuContent,
    ContextMenuItem,
    ContextMenuSeparator,
} from "./ui/context-menu"
