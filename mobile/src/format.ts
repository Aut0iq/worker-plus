/** 104 -> "1ч 44м", 0 -> "0м" */
export function fmtMinutes(m: number | null | undefined): string {
  const v = m == null ? 0 : m;
  if (v === 0) return '0м';
  const h = Math.trunc(v / 60);
  const mm = Math.abs(v % 60);
  return h !== 0 ? `${h}ч ${mm}м` : `${mm}м`;
}

/** 3725 -> "01:02:05" */
export function fmtClock(totalSeconds: number): string {
  const s = Math.max(0, Math.floor(totalSeconds));
  const p = (n: number) => String(n).padStart(2, '0');
  return `${p(Math.floor(s / 3600))}:${p(Math.floor((s % 3600) / 60))}:${p(s % 60)}`;
}

/** ISO timestamp -> "16.11.2026 19:35" in the app timezone */
export function fmtTs(iso: string | null | undefined, tz: string): string {
  if (!iso) return '—';
  return new Intl.DateTimeFormat('ru-RU', {
    timeZone: tz,
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
    .format(new Date(iso))
    .replace(',', '');
}
