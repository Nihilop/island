// Note rapide persistée (module : un seul état partagé, chargé une fois).
import { persisted } from "../../notch/persist";

export const { state: note } = persisted("note", { text: "" });
