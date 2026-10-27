import type { Index, Report, Rule } from './types';

import indexJson from './demo/index.json';
import naiveJson from './demo/naive.json';
import hardenedJson from './demo/hardened.json';
import catalogueJson from './demo/catalogue.json';

export interface Dataset {
  index: Index;
  reports: Record<string, Report>;
  rules: Rule[];
  /** true when the data came from `leakbench serve`, false when bundled. */
  live: boolean;
}

const bundled: Dataset = {
  index: indexJson as unknown as Index,
  reports: {
    naive: naiveJson as unknown as Report,
    hardened: hardenedJson as unknown as Report,
  },
  rules: catalogueJson as unknown as Rule[],
  live: false,
};

/**
 * Prefer the live API (so `leakbench scan` results appear on refresh), fall
 * back to the bundle compiled into the app. The fallback is why this page
 * works when opened straight from disk.
 */
export async function loadDataset(): Promise<Dataset> {
  try {
    const base = new URL('./', document.baseURI);
    const indexRes = await fetch(new URL('api/index.json', base).toString());
    if (!indexRes.ok) throw new Error('no api');
    const index = (await indexRes.json()) as Index;
    const reports: Record<string, Report> = {};
    for (const entry of index.reports) {
      const res = await fetch(new URL(`api/reports/${entry.file}`, base).toString());
      if (res.ok) reports[entry.id] = (await res.json()) as Report;
    }
    if (Object.keys(reports).length === 0) throw new Error('empty');
    return { index, reports, rules: bundled.rules, live: true };
  } catch {
    return bundled;
  }
}

/** zats -> "1.23456789 ZEC", with a zats fallback for dust. */
export function zec(zat?: number | null): string {
  if (zat === undefined || zat === null) return '—';
  const negative = zat < 0;
  const abs = Math.abs(zat);
  const sign = negative ? '-' : '';
  if (abs < 100_000) return `${sign}${abs.toLocaleString('en-US')} zats`;
  const whole = Math.floor(abs / 100_000_000);
  const frac = abs % 100_000_000;
  if (frac === 0) return `${sign}${whole} ZEC`;
  const digits = String(frac).padStart(8, '0').replace(/0+$/, '');
  return `${sign}${whole}.${digits} ZEC`;
}

export function shortHash(hash: string, lead = 8, tail = 6): string {
  if (hash.length <= lead + tail + 1) return hash;
  return `${hash.slice(0, lead)}…${hash.slice(-tail)}`;
}

export function height(h?: number | null): string {
  return h === undefined || h === null ? 'mempool' : h.toLocaleString('en-US');
}

export function timeOf(entry: { time?: number | null }): string {
  if (!entry.time) return '';
  return new Date(entry.time * 1000).toISOString().replace('T', ' ').slice(0, 16) + 'Z';
}

export function pct(part: number, whole: number): string {
  if (whole <= 0) return '0%';
  return `${Math.round((part / whole) * 100)}%`;
}
