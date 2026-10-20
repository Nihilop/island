// Appels à l'API du magasin depuis le panneau : cookie de session (même origine) + en-tête
// `X-Island-Admin` (exigé par le serveur pour toute modification faite avec le cookie :
// protection CSRF).

export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
  ) {
    super(code);
  }
}

export async function api<T = unknown>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(path, {
    method,
    credentials: "same-origin",
    headers: { "X-Island-Admin": "1", ...(body !== undefined ? { "Content-Type": "application/json" } : {}) },
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  if (res.status === 204) return undefined as T;
  const data = await res.json().catch(() => null);
  if (!res.ok) throw new ApiError(res.status, data?.error ?? `http${res.status}`);
  return data as T;
}

export interface Page<T> {
  items: T[];
  page: number;
  total: number;
}

/** Interrupteur d'urgence des publications. */
export interface Publishing {
  open: boolean;
  message: string;
  updatedAt: string;
  updatedBy: string | null;
}

export interface Me {
  id: number;
  login: string;
  avatarUrl: string | null;
  role: "user" | "moderator" | "admin";
}

const MESSAGES: Record<string, string> = {
  unauthorized: "Session expirée : reconnecte-toi.",
  forbidden: "Action réservée à un autre rôle.",
  notFound: "Introuvable (déjà traité ?).",
  isAdmin: "L'admin ne peut pas être modifié.",
  cannotBanSelf: "Impossible de se bannir soi-même.",
  unknownGithubUser: "Aucun compte GitHub à ce nom.",
  unknownUser: "Aucun compte à ce nom.",
  invalidLogin: "Nom GitHub invalide.",
  notModerator: "Ce compte n'est pas modérateur.",
  csrf: "Requête refusée (protection CSRF).",
  publishingPaused: "Les publications sont suspendues.",
  noSigningKey: "Clé du magasin absente sur le serveur (STORE_SIGNING_KEY) : approbation impossible.",
  reasonRequired: "Un motif est obligatoire.",
  internal: "Erreur du serveur.",
};

export function message(e: unknown): string {
  if (e instanceof ApiError) return MESSAGES[e.code] ?? e.code;
  return String(e);
}

export const REASONS: Record<string, string> = {
  illegal: "Illégal",
  malware: "Malveillant",
  copyright: "Droits d'auteur",
  offensive: "Choquant",
  broken: "Ne marche pas",
  other: "Autre",
};

export function date(iso: string | null): string {
  if (!iso) return "—";
  return new Intl.DateTimeFormat("fr-FR", { dateStyle: "short", timeStyle: "short" }).format(new Date(iso));
}
