import { useEffect, useMemo, useState } from 'react';
import Findings from './Findings';
import { CompareView, FactsView, GraphView, TimelineView } from './Views';
import { height, loadDataset, zec, type Dataset } from './data';
import { VERDICT_COPY, type Report } from './types';
import { Metric, ScoreDial, SeverityBars } from './ui';

type View = 'findings' | 'timeline' | 'graph' | 'facts' | 'compare';

const TABS: { id: View; label: string }[] = [
  { id: 'findings', label: 'Findings' },
  { id: 'timeline', label: 'Timeline' },
  { id: 'graph', label: 'Pool map' },
  { id: 'facts', label: 'Facts' },
  { id: 'compare', label: 'Compare' },
];

/** Deep links: #<subject>/<view>, e.g. #hardened/compare. */
function parseHash(): { subject?: string; view?: View } {
  const raw = window.location.hash.replace(/^#\/?/, '');
  const [subject, view] = raw.split('/');
  return {
    subject: subject || undefined,
    view: TABS.some((t) => t.id === view) ? (view as View) : undefined,
  };
}

export default function App() {
  const initial = parseHash();
  const [data, setData] = useState<Dataset | null>(null);
  const [selected, setSelected] = useState<string>(initial.subject ?? 'naive');
  const [view, setView] = useState<View>(initial.view ?? 'findings');

  useEffect(() => {
    const next = `#${selected}/${view}`;
    if (window.location.hash !== next) window.history.replaceState(null, '', next);
  }, [selected, view]);

  useEffect(() => {
    let alive = true;
    loadDataset().then((set) => {
      if (alive) setData(set);
    });
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    if (data && !data.reports[selected]) {
      const first = Object.keys(data.reports)[0];
      if (first) setSelected(first);
    }
  }, [data, selected]);

  const order = useMemo(() => {
    if (!data) return [];
    const preferred = ['naive', 'hardened'];
    const rest = Object.keys(data.reports).filter((id) => !preferred.includes(id)).sort();
    return [...preferred.filter((id) => data.reports[id]), ...rest];
  }, [data]);

  if (!data) {
    return (
      <div className="boot">
        <span className="boot-mark">λ</span>
        <span>loading reports…</span>
      </div>
    );
  }

  const report: Report = data.reports[selected] ?? data.reports[order[0]];
  const reports = order.map((id) => data.reports[id]);

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark">λ</span>
          <span className="brand-name">LEAKBENCH</span>
          <span className="brand-ver">v{report.engine.version}</span>
        </div>
        <div className="topmeta">
          <span className="status" data-live={data.live}>
            <span className="status-dot" />
            {data.live ? 'live api' : 'bundled demo'}
          </span>
          <span className="sep">·</span>
          <span>{report.engine.heuristics} rules</span>
          <span className="sep">·</span>
          <span>{report.engine.block_interval_secs}s blocks</span>
          <span className="sep">·</span>
          <span className="src">{report.engine.fact_sources.join(' + ')}</span>
        </div>
      </header>

      <div className="shell">
        <aside className="rail">
          <div className="rail-head">subjects</div>
          {order.map((id) => {
            const r = data.reports[id];
            const fatal = r.findings.filter((f) => f.fatal).length;
            return (
              <button
                key={id}
                className="rail-item"
                data-active={id === selected}
                data-verdict={r.verdict}
                onClick={() => setSelected(id)}
              >
                <span className="rail-score">{r.score.value}</span>
                <span className="rail-text">
                  <strong>{r.subject.label}</strong>
                  <em>
                    {r.verdict}
                    {fatal > 0 ? ` · ${fatal} fatal` : ''} · {r.findings.length} findings
                  </em>
                </span>
              </button>
            );
          })}

          <div className="rail-head">rules</div>
          <ol className="rail-rules">
            {data.rules.map((rule) => (
              <li key={rule.id} title={rule.description}>
                <code>{rule.id}</code>
                <span>{rule.title}</span>
              </li>
            ))}
          </ol>

          <div className="rail-foot">
            <a href="https://github.com/raizo07/LeakBench" target="_blank" rel="noreferrer">
              source
            </a>
            <a href="api/index.json" target="_blank" rel="noreferrer">
              api
            </a>
            <a href="https://zips.z.cash/zip-0258" target="_blank" rel="noreferrer">
              ZIP 258
            </a>
          </div>
        </aside>

        <main className="main">
          <section className="hero" data-verdict={report.verdict}>
            <div className="hero-main">
              <div className="eyebrow">
                {report.subject.kind} · {report.subject.network} · {report.subject.tx_count} tx ·{' '}
                {height(report.subject.first_height)}–{height(report.subject.last_height)}
              </div>
              <h1>{report.subject.label}</h1>
              {report.subject.description && <p className="lede">{report.subject.description}</p>}

              <div className="verdict">
                <span className="verdict-title">{VERDICT_COPY[report.verdict].title}</span>
                <span className="verdict-sub">{VERDICT_COPY[report.verdict].sub}</span>
              </div>

              <div className="metrics">
                <Metric
                  label="fatal leaks"
                  value={String(report.findings.filter((f) => f.fatal).length)}
                  tone={report.findings.some((f) => f.fatal) ? 'crit' : 'mint'}
                />
                <Metric label="findings" value={String(report.findings.length)} />
                <Metric label="risk" value={report.score.risk_points.toFixed(1)} />
                <Metric
                  label="exposed value"
                  value={exposedValue(report)}
                  tone="muted"
                />
              </div>
            </div>

            <div className="hero-score">
              <ScoreDial
                value={report.score.value}
                grade={report.score.grade}
                verdict={report.verdict}
              />
              <SeverityBars by={report.score.by_severity} />
            </div>
          </section>

          <nav className="tabs" role="tablist">
            {TABS.map((tab) => (
              <button
                key={tab.id}
                role="tab"
                aria-selected={view === tab.id}
                data-active={view === tab.id}
                onClick={() => setView(tab.id)}
              >
                {tab.label}
                {tab.id === 'findings' && <span className="tab-count">{report.findings.length}</span>}
              </button>
            ))}
          </nav>

          {view === 'findings' && <Findings report={report} />}
          {view === 'timeline' && <TimelineView report={report} />}
          {view === 'graph' && <GraphView report={report} />}
          {view === 'facts' && <FactsView report={report} />}
          {view === 'compare' && <CompareView reports={reports} />}

          <footer className="foot">
            <p>{report.disclaimer}</p>
            <p className="foot-meta">
              report <code>{report.schema}</code> · generated {report.generated_at} · engine{' '}
              {report.engine.name} {report.engine.version}
            </p>
          </footer>
        </main>
      </div>
    </div>
  );
}

/** Sum of every amount a finding says is public, without double counting. */
function exposedValue(report: Report): string {
  const seen = new Set<string>();
  let total = 0;
  for (const finding of report.findings) {
    if (!finding.fatal) continue;
    for (const ev of finding.evidence) {
      if (!/amount/.test(ev.label)) continue;
      const key = `${finding.txids?.[0] ?? ''}:${ev.value}`;
      if (seen.has(key)) continue;
      seen.add(key);
      const match = /^(-?[\d.]+) ZEC$/.exec(ev.value);
      if (match) total += Math.round(parseFloat(match[1]) * 100_000_000);
    }
  }
  return total > 0 ? zec(total) : '—';
}
