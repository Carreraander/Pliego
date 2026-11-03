import { ht } from "./strings";

export function greeting(hour: number): "home.morning" | "home.afternoon" | "home.evening" {
  return hour < 13 ? "home.morning" : hour < 20 ? "home.afternoon" : "home.evening";
}

/** «hoy, 10:42», «ayer» o «3 nov. 2026». */
export function formatOpened(seconds: number, now = new Date(), loc?: string): string {
  const d = new Date(seconds * 1000);
  const startOfDay = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const days = Math.round((startOfDay(now) - startOfDay(d)) / 86_400_000);
  if (days === 0) return ht("recent.today", { time: new Intl.DateTimeFormat(loc, { hour: "2-digit", minute: "2-digit" }).format(d) });
  if (days === 1) return ht("recent.yesterday");
  return new Intl.DateTimeFormat(loc, { day: "numeric", month: "short", year: "numeric" }).format(d);
}
