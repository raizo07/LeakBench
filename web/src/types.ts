export type Severity = 'info' | 'low' | 'medium' | 'high' | 'critical';
export type Confidence = 'low' | 'medium' | 'high';
export type Category = 'leak' | 'linkage' | 'fingerprint' | 'hygiene';
export type Verdict = 'disqualified' | 'at-risk' | 'acceptable' | 'clean';
export type Pool = 'transparent' | 'sapling' | 'orchard' | 'ironwood';

export interface Evidence {
  label: string;
  value: string;
  detail?: string | null;
}

export interface Finding {
  id: string;
  title: string;
  severity: Severity;
  confidence: Confidence;
  category: Category;
  fatal: boolean;
  summary: string;
  evidence: Evidence[];
  remediation: string;
  txids?: string[];
  references?: string[];
}

export interface Score {
  value: number;
  grade: string;
  verdict: Verdict;
  risk_points: number;
  by_severity: Record<string, number>;
}

export interface PoolFacts {
  spends: number;
  outputs: number;
  value_balance_zat?: number | null;
  value_in_zat?: number | null;
  value_out_zat?: number | null;
  addresses?: string[];
}

export interface TxFacts {
  txid: string;
  version: number;
  block_height?: number | null;
  block_time?: number | null;
  fee_zat?: number | null;
  transparent: PoolFacts;
  sapling: PoolFacts;
  orchard: PoolFacts;
  ironwood: PoolFacts;
  expiry_height?: number | null;
  anchor_age_blocks?: number | null;
  label?: string | null;
  source: 'fixture' | 'rpc' | 'raw' | 'simulated' | 'manual';
}

export interface WalletFacts {
  label: string;
  network: string;
  birthday_height: number;
  notes: {
    txid: string;
    pool: Pool;
    value_zat: number;
    received_height: number;
    memo_len: number;
    is_change: boolean;
  }[];
  addresses: { address: string; pool: Pool; uses: number; first_height: number; last_height: number }[];
}

export interface TimelineEntry {
  height?: number | null;
  time?: number | null;
  txid: string;
  label: string;
  shape: string;
  severity?: Severity | null;
}

export interface Report {
  schema: string;
  generated_at: string;
  engine: {
    name: string;
    version: string;
    heuristics: number;
    fact_sources: string[];
    network: string;
    block_interval_secs: number;
  };
  subject: {
    label: string;
    kind: 'transaction' | 'wallet' | 'scenario';
    network: string;
    tx_count: number;
    first_height?: number | null;
    last_height?: number | null;
    description?: string | null;
    has_wallet_facts: boolean;
  };
  verdict: Verdict;
  score: Score;
  findings: Finding[];
  timeline: TimelineEntry[];
  facts: { transactions: TxFacts[]; wallet?: WalletFacts | null };
  disclaimer: string;
}

export interface IndexEntry {
  file: string;
  id: string;
  label: string;
  kind: string;
  description?: string | null;
  score: number;
  grade: string;
  verdict: Verdict;
  fatal: number;
  findings: number;
  by_severity: Record<string, number>;
  tx_count: number;
  generated_at: string;
}

export interface Index {
  reports: IndexEntry[];
}

export interface Rule {
  id: string;
  title: string;
  description: string;
}

export const SEVERITY_ORDER: Severity[] = ['critical', 'high', 'medium', 'low', 'info'];

export const VERDICT_COPY: Record<Verdict, { title: string; sub: string }> = {
  disqualified: {
    title: 'Leaks on chain',
    sub: 'At least one finding is a disclosure the brief treats as disqualifying, not a deduction.',
  },
  'at-risk': {
    title: 'Material leaks',
    sub: 'Nothing fatal, but the residual fingerprint is strong enough to single this subject out.',
  },
  acceptable: {
    title: 'Residual disclosure only',
    sub: 'The unavoidable protocol-level disclosures are present; nothing here identifies the subject by itself.',
  },
  clean: {
    title: 'No material leaks',
    sub: 'Nothing in the catalogue can see this subject. That is a statement about the catalogue, not a proof.',
  },
};

export const POOL_LABEL: Record<Pool, string> = {
  transparent: 'Transparent',
  sapling: 'Sapling',
  orchard: 'Orchard',
  ironwood: 'Ironwood',
};
