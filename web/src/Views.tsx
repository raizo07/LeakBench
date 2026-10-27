import { height, shortHash, timeOf, zec } from './data';
import { POOL_LABEL, type Pool, type Report, type TxFacts } from './types';

const POOLS: Pool[] = ['transparent', 'sapling', 'orchard', 'ironwood'];

const SEV_VAR: Record<string, string> = {
  critical: 'var(--crit)',
  high: 'var(--high)',
  medium: 'var(--med)',
  low: 'var(--low)',
  info: 'var(--info)',
};

function dotColor(severity?: string | null): string {
  return severity ? SEV_VAR[severity] ?? 'var(--muted)' : 'var(--line-2)';
}

export function TimelineView({ report }: { report: Report }) {
  const heights = report.timeline
    .map((entry) => entry.height)
    .filter((h): h is number => typeof h === 'number');
  const min = heights.length ? Math.min(...heights) : 0;
  const max = heights.length ? Math.max(...heights) : 1;
  const span = Math.max(1, max - min);

  return (
    <section className="panel">
      <div className="panel-head">
        <div>
          <h2>Timeline</h2>
          <p>
            Height-ordered activity. The rail is sequence, not scale — read the gap column for
            real distance. Clusters are free to identify: at {report.engine.block_interval_secs}s
            per block, five blocks is{' '}
            {Math.round((report.engine.block_interval_secs * 5) / 60)} minutes of wall clock.
          </p>
        </div>
        <div className="legend">
          {(['critical', 'high', 'medium', 'low'] as const).map((s) => (
            <span className="legend-item" key={s}>
              <i style={{ background: SEV_VAR[s] }} />
              {s}
            </span>
          ))}
        </div>
      </div>

      <div className="tl-rail" aria-hidden="true">
        {report.timeline.map((entry, i) => (
          <span
            key={`${entry.txid}-${i}`}
            className="tl-tick"
            style={{
              left: `${(i / Math.max(1, report.timeline.length - 1)) * 100}%`,
              background: dotColor(entry.severity),
            }}
            title={`${height(entry.height)} · ${entry.label}`}
          />
        ))}
      </div>
      <div className="tl-scale">
        <span>{height(min)}</span>
        <span>sequence · {span} blocks total</span>
        <span>{height(max)}</span>
      </div>

      <table className="table">
        <thead>
          <tr>
            <th>height</th>
            <th>gap</th>
            <th>time (UTC)</th>
            <th>what</th>
            <th>shape</th>
            <th>worst</th>
          </tr>
        </thead>
        <tbody>
          {report.timeline.map((entry, i) => (
            <tr key={`${entry.txid}-${i}`}>
              <td className="mono">{height(entry.height)}</td>
              <td className="mono dim">
                {i === 0 || entry.height == null || report.timeline[i - 1].height == null
                  ? '—'
                  : `+${entry.height! - report.timeline[i - 1].height!}`}
              </td>
              <td className="mono dim">{timeOf(entry) || '—'}</td>
              <td>
                {entry.label}
                <code className="tx-hint" title={entry.txid}>
                  {shortHash(entry.txid, 6, 4)}
                </code>
              </td>
              <td className="mono dim">{entry.shape}</td>
              <td>
                {entry.severity ? (
                  <span className="sev-text" style={{ color: SEV_VAR[entry.severity] }}>
                    {entry.severity}
                  </span>
                ) : (
                  <span className="dim">—</span>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}

export function GraphView({ report }: { report: Report }) {
  const rows = report.timeline;
  const byId = new Map<string, TxFacts>(report.facts.transactions.map((tx) => [tx.txid, tx]));

  const LEFT = 158;
  const COLW = 172;
  const TOP = 64;
  const ROWH = 44;
  const W = LEFT + COLW * 3 + 132;
  const H = TOP + Math.max(1, rows.length) * ROWH + 18;
  const xOf = (pool: Pool) => LEFT + POOLS.indexOf(pool) * COLW + COLW / 2;
  // Transparent bundles carry no `value_balance_zat`, so derive one for the
  // diagram. This is presentation only; the rules read the real fields.
  const balanceOf = (tx: TxFacts, pool: Pool): number => {
    const direct = tx[pool].value_balance_zat ?? 0;
    if (pool !== 'transparent' || direct !== 0) return direct;
    return (tx.transparent.value_in_zat ?? 0) - (tx.transparent.value_out_zat ?? 0);
  };

  let crossings = 0;

  return (
    <section className="panel">
      <div className="panel-head">
        <div>
          <h2>Pool map</h2>
          <p>
            Rows are transactions, columns are value pools. A line means value crossed pools; between
            two shielded columns that is the turnstile, and the amount on the line is the value
            balance consensus publishes.
          </p>
        </div>
        <div className="legend">
          <span className="legend-item">
            <i className="lg-dot" style={{ background: 'var(--muted)' }} />
            pool activity
          </span>
          <span className="legend-item">
            <i className="lg-line" style={{ background: 'var(--crit)' }} />
            fatal crossing
          </span>
          <span className="legend-item">
            <i className="lg-line" style={{ background: 'var(--med)' }} />
            disclosed amount
          </span>
        </div>
      </div>

      <div className="graph-wrap">
        <svg className="graph" viewBox={`0 0 ${W} ${H}`} role="img" aria-label="Pool activity map">
          {POOLS.map((pool, i) => (
            <g key={pool}>
              <text x={LEFT + i * COLW + COLW / 2} y={26} textAnchor="middle" className="g-col">
                {POOL_LABEL[pool]}
              </text>
              <line
                x1={LEFT + i * COLW + COLW / 2}
                y1={38}
                x2={LEFT + i * COLW + COLW / 2}
                y2={H - 12}
                className="g-grid"
              />
            </g>
          ))}

          {rows.map((entry, i) => {
            const tx = byId.get(entry.txid);
            const y = TOP + i * ROWH + ROWH / 2;
            if (!tx) return null;

            const active = POOLS.filter((p) => tx[p].spends + tx[p].outputs > 0);
            const leaving = POOLS.find((p) => balanceOf(tx, p) > 0);
            const entering = POOLS.find((p) => balanceOf(tx, p) < 0);
            const crossing = leaving && entering && leaving !== entering;
            if (crossing) crossings += 1;
            const amount = crossing
              ? Math.min(Math.abs(balanceOf(tx, leaving)), Math.abs(balanceOf(tx, entering)))
              : 0;
            const stroke = crossing ? dotColor(entry.severity) : 'var(--line-2)';

            return (
              <g key={`${entry.txid}-${i}`}>
                <text x={12} y={y - 2} className="g-label">
                  {entry.label.length > 24 ? `${entry.label.slice(0, 23)}…` : entry.label}
                </text>
                <text x={12} y={y + 11} className="g-sub">
                  {height(entry.height)}
                </text>

                {crossing && (
                  <>
                    <path
                      d={`M ${xOf(leaving)} ${y} C ${xOf(leaving) + 46} ${y}, ${xOf(entering) - 46} ${y}, ${xOf(entering)} ${y}`}
                      fill="none"
                      stroke={stroke}
                      strokeWidth={entry.severity === 'critical' ? 2.4 : 1.6}
                      strokeDasharray={entry.severity === 'critical' ? '0' : '4 3'}
                    />
                    <text
                      x={(xOf(leaving) + xOf(entering)) / 2}
                      y={y - 7}
                      textAnchor="middle"
                      className="g-amount"
                      fill={stroke}
                    >
                      {zec(amount)}
                    </text>
                  </>
                )}

                {active.map((pool) => {
                  const facts = tx[pool];
                  const actions = facts.spends + facts.outputs;
                  const r = 3.5 + Math.min(5, actions);
                  return (
                    <g key={pool}>
                      <circle
                        cx={xOf(pool)}
                        cy={y}
                        r={r}
                        fill={dotColor(entry.severity)}
                        opacity={0.85}
                      />
                      <title>{`${POOL_LABEL[pool]}: ${facts.spends} spend / ${facts.outputs} output`}</title>
                    </g>
                  );
                })}
              </g>
            );
          })}
        </svg>
      </div>

      <p className="graph-note">
        {crossings} crossing{crossings === 1 ? '' : 's'} in this subject.{' '}
        {crossings > 0
          ? 'Each one published its amount; the question the score answers is whether that amount identified anyone.'
          : 'No turnstile activity: nothing here straddles two shielded pools.'}
      </p>
    </section>
  );
}

export function FactsView({ report }: { report: Report }) {
  const pools: Pool[] = ['transparent', 'sapling', 'orchard', 'ironwood'];
  return (
    <section className="panel">
      <div className="panel-head">
        <div>
          <h2>Facts</h2>
          <p>
            The normalized observation set the engine ran on. Everything here is either
            consensus-public or supplied by the subject&apos;s own scan; nothing was guessed.
          </p>
        </div>
        <div className="legend">
          <span className="legend-item">
            <i style={{ background: 'var(--mint)' }} /> {report.facts.transactions.length} parsed
          </span>
        </div>
      </div>

      <div className="table-scroll">
        <table className="table facts">
          <thead>
            <tr>
              <th>height</th>
              <th>v</th>
              {pools.map((p) => (
                <th key={p}>{POOL_LABEL[p].slice(0, 4).toLowerCase()}</th>
              ))}
              <th>fee</th>
              <th>orchard vb</th>
              <th>ironwood vb</th>
              <th>label</th>
            </tr>
          </thead>
          <tbody>
            {report.facts.transactions.map((tx) => (
              <tr key={tx.txid}>
                <td className="mono">{height(tx.block_height)}</td>
                <td className="mono dim">{tx.version}</td>
                {pools.map((p) => {
                  const f = tx[p];
                  const active = f.spends + f.outputs > 0;
                  return (
                    <td key={p} className="mono">
                      {active ? (
                        <span className="pair">
                          {f.spends}/{f.outputs}
                        </span>
                      ) : (
                        <span className="dim">·</span>
                      )}
                    </td>
                  );
                })}
                <td className="mono">{tx.fee_zat ? zec(tx.fee_zat) : <span className="dim">—</span>}</td>
                <td className="mono dim">{tx.orchard.value_balance_zat ? zec(tx.orchard.value_balance_zat) : '—'}</td>
                <td className="mono dim">{tx.ironwood.value_balance_zat ? zec(tx.ironwood.value_balance_zat) : '—'}</td>
                <td className="dim">{tx.label ?? '—'}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {report.facts.wallet && (
        <div className="wallet">
          <h3>Wallet-side facts</h3>
          <p className="dim">
            {report.facts.wallet.notes.length} notes ·{' '}
            {report.facts.wallet.addresses.length} transparent address(es) · birthday{' '}
            {height(report.facts.wallet.birthday_height)}
          </p>
          <table className="table">
            <thead>
              <tr>
                <th>pool</th>
                <th>value</th>
                <th>received</th>
                <th>memo</th>
                <th>tx</th>
              </tr>
            </thead>
            <tbody>
              {report.facts.wallet.notes.slice(0, 12).map((note, i) => (
                <tr key={`${note.txid}-${i}`}>
                  <td>{POOL_LABEL[note.pool]}</td>
                  <td className="mono">{zec(note.value_zat)}</td>
                  <td className="mono dim">{height(note.received_height)}</td>
                  <td className="mono dim">{note.memo_len ? `${note.memo_len}B` : '—'}</td>
                  <td className="mono dim">{shortHash(note.txid, 6, 4)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

export function CompareView({ reports }: { reports: Report[] }) {
  if (reports.length < 2) {
    return (
      <section className="panel">
        <div className="empty">
          <strong>Nothing to compare yet.</strong>
          <p>Run `leakbench demo` to generate more than one subject.</p>
        </div>
      </section>
    );
  }

  const sorted = [...reports].sort((a, b) => a.score.value - b.score.value);
  const low = sorted[0];
  const high = sorted[sorted.length - 1];
  const idsOf = (r: Report) => new Set(r.findings.map((f) => f.id));
  const lowIds = idsOf(low);
  const highIds = idsOf(high);
  const removed = [...lowIds].filter((id) => !highIds.has(id)).sort();
  const remaining = [...lowIds].filter((id) => highIds.has(id)).sort();
  const titleOf = (r: Report, id: string) => r.findings.find((f) => f.id === id)?.title ?? id;
  const fatalLow = low.findings.filter((f) => f.fatal).length;
  const fatalHigh = high.findings.filter((f) => f.fatal).length;

  return (
    <section className="panel">
      <div className="panel-head">
        <div>
          <h2>Compare</h2>
          <p>
            Both subjects start from the same position: an Orchard balance received from payroll.
            The only difference is how it crossed into Ironwood.
          </p>
        </div>
        <div className="delta-badge">
          <span>{low.score.value}</span>
          <i>→</i>
          <span>{high.score.value}</span>
          <em>+{high.score.value - low.score.value}</em>
        </div>
      </div>

      <div className="compare-grid">
        {[low, high].map((r) => (
          <div className="compare-card" data-verdict={r.verdict} key={r.subject.label}>
            <div className="cc-head">
              <strong>{r.subject.label}</strong>
              <span>{r.verdict}</span>
            </div>
            <div className="cc-score">
              {r.score.value}
              <em>/100</em>
              <span className="cc-grade">{r.score.grade}</span>
            </div>
            <div className="cc-meta">
              {r.findings.length} findings · {r.findings.filter((f) => f.fatal).length} fatal
            </div>
          </div>
        ))}
      </div>

      <div className="compare-lists">
        <div className="compare-list">
          <h3>
            Removed by hardening <span>{removed.length}</span>
          </h3>
          {removed.length === 0 && <p className="dim">Nothing removed.</p>}
          <ul>
            {removed.map((id) => (
              <li key={id}>
                <code>{id}</code>
                <span>{titleOf(low, id)}</span>
              </li>
            ))}
          </ul>
        </div>
        <div className="compare-list">
          <h3>
            Still present <span>{remaining.length}</span>
          </h3>
          <p className="dim">
            {remaining.length === 0
              ? 'No finding survives in both — the two wallets share no failure mode.'
              : 'These are the disclosures that chunking cannot remove.'}
          </p>
          <ul>
            {remaining.map((id) => (
              <li key={id}>
                <code>{id}</code>
                <span>{titleOf(high, id)}</span>
              </li>
            ))}
          </ul>
        </div>
      </div>

      <div className="compare-foot">
        fatal findings {fatalLow} → {fatalHigh} · the residual is a protocol-level disclosure, not a
        wallet bug. That distinction is the point of the tool.
      </div>
    </section>
  );
}
