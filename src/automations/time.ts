// Plages horaires des automatisations (pur, testable).

const minutesOf = (hm: string) => {
  const [h, m] = hm.split(":").map(Number);
  return (h || 0) * 60 + (m || 0);
};

/** Plage horaire, éventuellement à cheval sur minuit (le jour compté est celui du début). */
export function inTimeRange(tr: { from: string; to: string; days: number[] }, d: Date): boolean {
  const cur = d.getHours() * 60 + d.getMinutes();
  const from = minutesOf(tr.from);
  const to = minutesOf(tr.to);
  const day = d.getDay();
  const days = tr.days.length ? tr.days : [0, 1, 2, 3, 4, 5, 6];
  if (from === to) return days.includes(day);
  if (from < to) return days.includes(day) && cur >= from && cur < to;
  // Ex. 22:00 → 07:00 : soir du jour J, ou matin du lendemain d'un jour J.
  if (cur >= from) return days.includes(day);
  return cur < to && days.includes((day + 6) % 7);
}
