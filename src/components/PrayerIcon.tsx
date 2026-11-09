import { Moon, MoonStar, Sun, SunDim, Sunset, type LucideProps } from "lucide-react";
import type { Prayer } from "../lib/types";

/** The sky at each prayer — one line-icon family, distinct silhouettes:
 * stars before dawn, full sun at noon, a lowered sun in the afternoon. */
const ICONS = {
  fajr: MoonStar,
  dhuhr: Sun,
  asr: SunDim,
  maghrib: Sunset,
  isha: Moon,
} satisfies Record<Prayer, unknown>;

export function PrayerIcon({ prayer, ...props }: { prayer: Prayer } & LucideProps) {
  const Icon = ICONS[prayer];
  return <Icon aria-hidden strokeWidth={1.75} {...props} />;
}
