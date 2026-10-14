// To-dos persistés (module : un seul état partagé, chargé une fois).
import { persisted } from "../../notch/persist";

export interface Todo {
  id: string;
  text: string;
  done: boolean;
  starred: boolean;
}

export const { state: todos } = persisted("todos", { items: [] as Todo[] });
