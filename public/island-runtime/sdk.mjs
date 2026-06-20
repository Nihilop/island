// Ré-exporte le SDK de l'hôte (même instance) pour les extensions installées.
// Doit lister les exports VALEUR de src/sdk/index.ts (les types sont effacés).
const s = window.__ISLAND_SDK__;

export const defineExtension = s.defineExtension;
export const useIsland = s.useIsland;

export const Button = s.Button;
export const buttonVariants = s.buttonVariants;
export const Switch = s.Switch;
export const Progress = s.Progress;
export const Kbd = s.Kbd;
export const KbdGroup = s.KbdGroup;

export const Select = s.Select;
export const SelectContent = s.SelectContent;
export const SelectGroup = s.SelectGroup;
export const SelectItem = s.SelectItem;
export const SelectItemText = s.SelectItemText;
export const SelectLabel = s.SelectLabel;
export const SelectScrollDownButton = s.SelectScrollDownButton;
export const SelectScrollUpButton = s.SelectScrollUpButton;
export const SelectSeparator = s.SelectSeparator;
export const SelectTrigger = s.SelectTrigger;
export const SelectValue = s.SelectValue;

export default s;
