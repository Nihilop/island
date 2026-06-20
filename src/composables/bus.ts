// Bus pub/sub INTER-EXTENSIONS. Permet à une extension « producteur » (now-playing,
// présence, build status…) d'être consommée par d'autres, sans couplage direct.
// Purement front (toutes les extensions partagent le même realm via le pont hôte).
// Abonnements tracés par `owner` (= id d'extension) → nettoyage auto à la désactivation.

type Handler = (payload: any) => void;
interface Sub {
  channel: string;
  handler: Handler;
  owner: string;
}

const subs: Sub[] = [];

export function busEmit(channel: string, payload: unknown) {
  // Copie défensive : un handler peut se désabonner pendant l'émission.
  for (const s of subs.slice()) {
    if (s.channel === channel) {
      try {
        s.handler(payload);
      } catch {
        /* un abonné qui throw ne casse pas les autres */
      }
    }
  }
}

export function busOn(channel: string, handler: Handler, owner = ""): () => void {
  const sub: Sub = { channel, handler, owner };
  subs.push(sub);
  return () => {
    const i = subs.indexOf(sub);
    if (i >= 0) subs.splice(i, 1);
  };
}

/** Retire tous les abonnements d'une extension (appelé à sa désactivation). */
export function busClear(owner: string) {
  for (let i = subs.length - 1; i >= 0; i--) {
    if (subs[i].owner === owner) subs.splice(i, 1);
  }
}
