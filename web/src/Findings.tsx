import { useState } from 'react';
import { shortHash } from './data';
import type { Report } from './types';
import { ConfidenceChip, SeverityChip } from './ui';

export default function Findings({ report }: { report: Report }) {
  const [fatalOnly, setFatalOnly] = useState(false);
  const [open, setOpen] = useState<Record<string, boolean>>({});

  const fatalCount = report.findings.filter((f) => f.fatal).length;
  const shown = fatalOnly ? report.findings.filter((f) => f.fatal) : report.findings;

  return (
    <section className="panel">
      <div className="panel-head">
        <div>
          <h2>Findings</h2>
          <p>
            Every finding names the observable it is derived from. Severity is about impact,
            confidence about evidence: a low-confidence finding is a question, not an accusation.
          </p>
        </div>
        <div className="filters">
          <button className="toggle" data-on={!fatalOnly} onClick={() => setFatalOnly(false)}>
            all <span>{report.findings.length}</span>
          </button>
          <button className="toggle" data-on={fatalOnly} onClick={() => setFatalOnly(true)}>
            fatal <span>{fatalCount}</span>
          </button>
        </div>
      </div>

      {shown.length === 0 && (
        <div className="empty">
          <strong>Nothing to report.</strong>
          <p>
            {fatalOnly
              ? 'No fatal disclosure was found. Switch to “all” to see fingerprints and hygiene.'
              : 'The catalogue found nothing it can see in this subject.'}
          </p>
        </div>
      )}

      <div className="findings">
        {shown.map((f, i) => {
          const key = `${f.id}:${i}`;
          const expanded = open[key] ?? i < 3;
          return (
            <article
              key={key}
              className="finding"
              data-sev={f.severity}
              data-fatal={f.fatal}
              data-open={expanded}
            >
              <div className="finding-head">
                <SeverityChip severity={f.severity} fatal={f.fatal} />
                <code className="fid">{f.id}</code>
                <h3>{f.title}</h3>
                <span className="cat">{f.category}</span>
                <ConfidenceChip confidence={f.confidence} />
                <button
                  className="chev"
                  aria-expanded={expanded}
                  aria-label={expanded ? 'collapse' : 'expand'}
                  onClick={() => setOpen((prev) => ({ ...prev, [key]: !expanded }))}
                >
                  {expanded ? '−' : '+'}
                </button>
              </div>

              <p className="finding-summary">{f.summary}</p>

              {expanded && (
                <>
                  <dl className="evidence">
                    {f.evidence.map((ev) => (
                      <div className="ev" key={`${ev.label}:${ev.value}`}>
                        <dt>{ev.label}</dt>
                        <dd>
                          <span className="mono">{ev.value}</span>
                          {ev.detail && <em>{ev.detail}</em>}
                        </dd>
                      </div>
                    ))}
                  </dl>

                  <div className="fix">
                    <span className="fix-label">fix</span>
                    <p>{f.remediation}</p>
                  </div>

                  {(f.txids?.length || f.references?.length) && (
                    <div className="refs">
                      {f.txids?.map((txid) => (
                        <span className="ref tx" key={txid} title={txid}>
                          tx {shortHash(txid)}
                        </span>
                      ))}
                      {f.references?.map((ref) => (
                        <a key={ref} href={ref} target="_blank" rel="noreferrer">
                          {ref.replace(/^https?:\/\//, '').replace(/\/$/, '')}
                        </a>
                      ))}
                    </div>
                  )}
                </>
              )}
            </article>
          );
        })}
      </div>
    </section>
  );
}
