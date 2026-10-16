// Icônes des dépenses : une clé courte stockée avec la dépense → composant Phosphor. Les
// données restent indépendantes de la librairie d'icônes.

import type { Component } from "vue";
import {
  PhAirplane,
  PhBank,
  PhBarbell,
  PhBookOpen,
  PhBus,
  PhCar,
  PhCloud,
  PhCoffee,
  PhCreditCard,
  PhDeviceMobile,
  PhDog,
  PhDrop,
  PhFilmStrip,
  PhFirstAid,
  PhFlame,
  PhForkKnife,
  PhGameController,
  PhGasPump,
  PhGift,
  PhGraduationCap,
  PhHouse,
  PhLightning,
  PhMusicNotes,
  PhPiggyBank,
  PhPill,
  PhReceipt,
  PhShieldCheck,
  PhShoppingCart,
  PhTShirt,
  PhTag,
  PhTelevision,
  PhTrain,
  PhWifiHigh,
} from "@phosphor-icons/vue";
import type { Category } from "./store";

export const ICONS: Record<string, Component> = {
  house: PhHouse,
  lightning: PhLightning,
  drop: PhDrop,
  flame: PhFlame,
  wifi: PhWifiHigh,
  mobile: PhDeviceMobile,
  tv: PhTelevision,
  music: PhMusicNotes,
  film: PhFilmStrip,
  game: PhGameController,
  cloud: PhCloud,
  cart: PhShoppingCart,
  food: PhForkKnife,
  coffee: PhCoffee,
  car: PhCar,
  fuel: PhGasPump,
  train: PhTrain,
  bus: PhBus,
  plane: PhAirplane,
  health: PhFirstAid,
  pill: PhPill,
  sport: PhBarbell,
  book: PhBookOpen,
  school: PhGraduationCap,
  pet: PhDog,
  gift: PhGift,
  clothes: PhTShirt,
  insurance: PhShieldCheck,
  bank: PhBank,
  card: PhCreditCard,
  savings: PhPiggyBank,
  bill: PhReceipt,
  tag: PhTag,
};

/** Icône par défaut d'une catégorie (choix rapide). */
export const CATEGORY_ICON: Record<string, string> = {
  home: "house",
  food: "cart",
  transport: "car",
  subscriptions: "tv",
  bills: "lightning",
  insurance: "insurance",
  health: "health",
  leisure: "game",
  shopping: "clothes",
  other: "tag",
};

/** Icône proposée pour une catégorie (catégorie créée → étiquette). */
export const categoryIcon = (c: Category) => CATEGORY_ICON[c] ?? "tag";

export const iconOf = (key: string): Component => ICONS[key] ?? PhTag;
