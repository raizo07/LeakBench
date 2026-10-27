import type { Severity, Verdict } from './types';

export const SEV_LABEL: Record<Severity, string> = {
  critical: 'critical',
  high: 'high',
  medium: 'medium',
  low: 'low',
  info: 'info',
};

const VERDICT_COLOR: Record<Verdict, string> = {
  disqualified: 'var(--crit)',
  'at-risk': 'var(--high)',
  acceptable: 'var(--med)',
  clean: 'var(--mint)',
};

export function SeverityChip({ severity, fatal }: { severity: Severity; fatal?: boolean }) {
  return (
    <span className="chip sev" data-sev={severity}>
      {fatal ? 'fatal ' : ''}
      {SEV_LABEL[severity]}
    </span>
  );
}

export function ConfidenceChip({ confidence }: { confidence: 'low' | 'medium' | 'high' }) {
  return (
    <span className="chip conf" data-conf={confidence} title={`${confidence} confidence`}>
      {confidence}
    </span>
  );
}

/** Arc dial. Color comes from the verdict, not the number: a 79 that is
 *  "acceptable" and a 79 that is "at-risk" should not look identical. */
export function ScoreDial({
  value,
  grade,
  verdict,
  size = 150,
}: {
  value: number;
  grade: string;
  verdict: Verdict;
  size?: number;
}) {
  const stroke = 9;
  const radius = (size - stroke * 2) / 2;
  const circumference = 2 * Math.PI * radius;
  // A zero score must still paint: an invisible arc reads as a rendering bug.
  const fraction = Math.max(0.015, Math.max(0, Math.min(100, value)) / 100);
  const dash = fraction * circumference;
  const color = VERDICT_COLOR[verdict];
  return (
    <svg className="dial" viewBox={`0 0 ${size} ${size}`} width={size} height={size} role="img" aria-label={`Score ${value} of 100, grade ${grade}`}>
      <circle cx={size / 2} cy={size / 2} r={radius} fill="none" stroke="var(--line-2)" strokeWidth={stroke} />
      <circle
        cx={size / 2}
        cy={size / 2}
        r={radius}
        fill="none"
        stroke={color}
        strokeWidth={stroke}
        strokeLinecap="round"
        strokeDasharray={`${dash} ${circumference}`}
        transform={`rotate(-90 ${size / 2} ${size / 2})`}
      />
      <text x={size / 2} y={size / 2 - 2} textAnchor="middle" className="dial-value" fill="var(--ink)">
        {value}
      </text>
      <text x={size / 2} y={size / 2 + 22} textAnchor="middle" className="dial-grade" fill={color}>
        {grade}
      </text>
      <text x={size / 2} y={size / 2 + 38} textAnchor="middle" className="dial-sub" fill="var(--muted)">
        / 100
      </text>
    </svg>
  );
}

const BARS: { key: Severity; color: string }[] = [
  { key: 'critical', color: 'var(--crit)' },
  { key: 'high', color: 'var(--high)' },
  { key: 'medium', color: 'var(--med)' },
  { key: 'low', color: 'var(--low)' },
  { key: 'info', color: 'var(--info)' },
];

export function SeverityBars({ by }: { by: Record<string, number> }) {
  const max = Math.max(1, ...BARS.map((b) => by[b.key] ?? 0));
  return (
    <div className="bars">
      {BARS.map(({ key, color }) => {
        const count = by[key] ?? 0;
        return (
          <div className="bar-row" key={key}>
            <span className="bar-label">{key}</span>
            <span className="bar-track">
              <span
                className="bar-fill"
                style={{ width: `${(count / max) * 100}%`, background: color, opacity: count ? 1 : 0.18 }}
              />
            </span>
            <span className="bar-count">{count}</span>
          </div>
        );
      })}
    </div>
  );
}

export function Metric({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone?: 'crit' | 'mint' | 'muted';
}) {
  return (
    <div className="metric">
      <span className="metric-label">{label}</span>
      <span className="metric-value" data-tone={tone}>
        {value}
      </span>
    </div>
  );
}
