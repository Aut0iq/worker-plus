import dayjs, { type Dayjs } from 'dayjs';
import utc from 'dayjs/plugin/utc';
import timezone from 'dayjs/plugin/timezone';

dayjs.extend(utc);
dayjs.extend(timezone);

let tz = 'UTC';
export const setAppTimezone = (v: string) => {
  tz = v;
};
export const getAppTimezone = () => tz;

type Min = number | bigint | null | undefined;
const num = (v: Min): number => (v == null ? 0 : Number(v));

/** 104 -> "1ч 44м", 0 -> "0м" */
export function fmtMinutes(m: Min): string {
  const v = num(m);
  if (v === 0) return '0м';
  const h = Math.trunc(v / 60);
  const mm = Math.abs(v % 60);
  return h !== 0 ? `${h}ч ${mm}м` : `${mm}м`;
}

export const numMin = num;

export function fmtSeconds(s: Min): string {
  const v = s == null ? null : Number(s);
  if (v == null) return '—';
  const h = Math.trunc(v / 3600);
  const m = Math.trunc((v % 3600) / 60);
  const sec = v % 60;
  if (h) return `${h}ч ${m}м ${sec}с`;
  if (m) return `${m}м ${sec}с`;
  return `${sec}с`;
}

export const inTz = (v?: string | null): Dayjs | null => (v ? dayjs(v).tz(tz) : null);
export const fmtTs = (v?: string | null): string => {
  const d = inTz(v);
  return d ? d.format('DD.MM.YYYY HH:mm:ss') : '—';
};
export const fmtDate = (v?: string | null): string => {
  const d = inTz(v);
  return d ? d.format('DD.MM.YYYY') : '—';
};
