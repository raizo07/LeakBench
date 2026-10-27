//! `leakbench` — analyze Zcash transactions for privacy leaks.

mod render;
mod schema;
mod serve;

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use leakbench_chain::{decode, devnet, fixtures, rpc::Rpc};
use leakbench_core::{analyze, Report, Severity, Subject, SubjectKind, WalletFacts};
use serde_json::Value;

#[derive(Parser)]
#[command(
    name = "leakbench",
    version,
    about = "Privacy leak analyzer for Zcash: facts in, findings out",
    long_about = "LeakBench scores Zcash transactions and wallets against a catalogue of \
                  on-chain leak heuristics: turnstile amount disclosure, transparent legs, fee \
                  fingerprints, timing clusters, address reuse and more.\n\n\
                  Facts come from fixtures, a local node's RPC, or the built-in simulator. It \
                  never uploads viewing keys and never needs a remote server."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the built-in naive and hardened scenarios and write reports.
    Demo {
        #[arg(long, default_value = "reports")]
        out: PathBuf,
        /// Also write the bundle the web UI imports (for a static build).
        #[arg(long)]
        web: Option<PathBuf>,
    },
    /// Analyze one built-in scenario.
    Simulate {
        #[arg(long, default_value = "naive")]
        scenario: String,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Analyze a transaction or wallet fact bundle.
    Scan {
        /// A JSON Subject (see docs/FACTS.md).
        #[arg(long, group = "src")]
        fixture: Option<PathBuf>,
        /// Output of `getrawtransaction <txid> 1`, or an array of them.
        #[arg(long, group = "src")]
        decoded: Option<PathBuf>,
        /// Local node RPC endpoint, e.g. http://127.0.0.1:18232
        #[arg(long, group = "src")]
        rpc: Option<String>,
        /// Transaction id to fetch from --rpc.
        #[arg(long, requires = "rpc")]
        txid: Option<String>,
        /// Wallet-side facts to attach (enables the wallet rules).
        #[arg(long)]
        wallet: Option<PathBuf>,
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Print the heuristic catalogue.
    Heuristics,
    /// Print the report JSON Schema.
    Schema,
    /// CI gate: exit non-zero when a report is over the threshold.
    Ci {
        #[arg(long)]
        report: PathBuf,
        /// critical | high | medium | low
        #[arg(long, default_value = "high")]
        fail_on: String,
    },
    /// Serve reports and the built web UI.
    Serve {
        #[arg(long, default_value = "reports")]
        dir: PathBuf,
        #[arg(long = "static", default_value = "web/dist")]
        static_dir: PathBuf,
        #[arg(long, default_value_t = 8787)]
        port: u16,
    },
    /// Check a node's RPC shape.
    Doctor {
        #[arg(long, default_value = "http://127.0.0.1:18232")]
        rpc: String,
    },
    /// Local-network recipes (and a ready-to-run zebrad config).
    Devnet {
        #[arg(long)]
        config: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) = run(cli) {
        eprintln!("\nerror: {err:#}");
        std::process::exit(2);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.cmd {
        Cmd::Demo { out, web } => demo(&out, web.as_deref()),
        Cmd::Simulate {
            scenario,
            out,
            json,
        } => {
            let sc = leakbench_sim::get(&scenario).ok_or_else(|| {
                anyhow::anyhow!("unknown scenario {scenario:?} (try: naive, hardened)")
            })?;
            let report = analyze(&sc.subject);
            emit(&report, out.as_deref(), json)
        }
        Cmd::Scan {
            fixture,
            decoded,
            rpc,
            txid,
            wallet,
            label,
            out,
            json,
        } => scan(fixture, decoded, rpc, txid, wallet, label, out, json),
        Cmd::Heuristics => {
            println!("\n  {:<8} {:<34} what it looks for\n", "id", "title");
            for (id, title, description) in leakbench_core::catalogue() {
                println!("  {id:<8} {title:<34} {description}");
            }
            println!(
                "\n  {} rules. Run `leakbench scan --fixture <f>` or `leakbench demo`.\n",
                leakbench_core::RULES.len()
            );
            Ok(())
        }
        Cmd::Schema => {
            let parsed: Value = serde_json::from_str(schema::SCHEMA)?;
            println!("{}", serde_json::to_string_pretty(&parsed)?);
            Ok(())
        }
        Cmd::Ci { report, fail_on } => ci(&report, &fail_on),
        Cmd::Serve {
            dir,
            static_dir,
            port,
        } => serve::run(dir, static_dir, port),
        Cmd::Doctor { rpc } => doctor(&rpc),
        Cmd::Devnet { config } => devnet_cmd(config),
    }
}

fn demo(out: &Path, web: Option<&Path>) -> Result<()> {
    std::fs::create_dir_all(out).with_context(|| format!("cannot create {}", out.display()))?;
    if let Some(web) = web {
        std::fs::create_dir_all(web).with_context(|| format!("cannot create {}", web.display()))?;
    }

    let mut reports = Vec::new();
    for sc in leakbench_sim::all() {
        let report = analyze(&sc.subject);
        write_json(&out.join(format!("{}.json", sc.id)), &report)?;
        if let Some(web) = web {
            write_json(&web.join(format!("{}.json", sc.id)), &report)?;
        }
        reports.push(report);
    }

    let index = index_json(&reports);
    std::fs::write(out.join("index.json"), &index)?;
    if let Some(web) = web {
        std::fs::write(web.join("index.json"), &index)?;
        let rules: Vec<Value> = leakbench_core::catalogue()
            .into_iter()
            .map(|(id, title, description)| {
                serde_json::json!({ "id": id, "title": title, "description": description })
            })
            .collect();
        std::fs::write(
            web.join("catalogue.json"),
            serde_json::to_string_pretty(&rules)?,
        )?;
    }

    println!("{}", render::compare_table(&reports));
    let naive = reports.iter().find(|r| r.subject.label.contains("naive"));
    let hardened = reports
        .iter()
        .find(|r| r.subject.label.contains("hardened"));
    if let (Some(n), Some(h)) = (naive, hardened) {
        println!(
            "\n  {:<22} {} -> {}  (+{} points)\n",
            "delta",
            n.score.value,
            h.score.value,
            h.score.value as i32 - n.score.value as i32
        );
    }
    println!("  reports  {}", out.display());
    println!("  next     leakbench serve --dir {}", out.display());
    if web.is_none() {
        println!("           (add --web web/src/demo to refresh the static UI bundle)");
    }
    println!();
    Ok(())
}

fn index_json(reports: &[Report]) -> String {
    let entries: Vec<Value> = reports
        .iter()
        .map(|r| {
            let id = r
                .subject
                .label
                .split_whitespace()
                .next()
                .unwrap_or("report")
                .to_string();
            serde_json::json!({
                "file": format!("{id}.json"),
                "id": id,
                "label": r.subject.label,
                "kind": r.subject.kind.as_str(),
                "description": r.subject.description,
                "score": r.score.value,
                "grade": r.score.grade,
                "verdict": r.verdict.as_str(),
                "fatal": r.fatal_count(),
                "findings": r.findings.len(),
                "by_severity": r.score.by_severity,
                "tx_count": r.subject.tx_count,
                "generated_at": r.generated_at,
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({ "reports": entries }))
        .unwrap_or_else(|_| "{\"reports\":[]}".into())
}

fn write_json(path: &Path, report: &Report) -> Result<()> {
    std::fs::write(path, report.to_json_pretty())
        .with_context(|| format!("cannot write {}", path.display()))
}

fn emit(report: &Report, out: Option<&Path>, json: bool) -> Result<()> {
    if let Some(path) = out {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        write_json(path, report)?;
    }
    if json {
        println!("{}", report.to_json_pretty());
    } else {
        print!("{}", render::render(report));
        if let Some(path) = out {
            println!("  wrote {}\n", path.display());
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn scan(
    fixture: Option<PathBuf>,
    decoded: Option<PathBuf>,
    rpc: Option<String>,
    txid: Option<String>,
    wallet: Option<PathBuf>,
    label: Option<String>,
    out: Option<PathBuf>,
    json: bool,
) -> Result<()> {
    let mut subject = if let Some(path) = fixture {
        fixtures::load_subject(&path)?
    } else if let Some(path) = decoded {
        subject_from_decoded(&path, label.as_deref())?
    } else if let Some(url) = rpc {
        let txid = txid.ok_or_else(|| anyhow::anyhow!("--rpc needs --txid"))?;
        let client = Rpc::parse(&url).map_err(|e| anyhow::anyhow!("{e}"))?;
        let raw = client
            .raw_transaction(&txid)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let facts =
            decode::tx_from_node_json(&raw, None, None).map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut s = Subject::new(
            label.unwrap_or_else(|| format!("tx {}", short(&txid))),
            SubjectKind::Transaction,
            "main",
        );
        s.txs.push(facts);
        s.context.tip_height = client.block_count().ok();
        s
    } else {
        bail!("give me facts: --fixture <json>, --decoded <json>, or --rpc <url> --txid <id>");
    };

    if let Some(path) = wallet {
        let facts: WalletFacts = fixtures::load_wallet(&path)?;
        subject.wallet = Some(facts);
    }

    let report = analyze(&subject);
    emit(&report, out.as_deref(), json)
}

fn subject_from_decoded(path: &Path, label: Option<&str>) -> Result<Subject> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let value: Value = serde_json::from_str(&text)?;
    let items: Vec<Value> = match value {
        Value::Array(items) => items,
        Value::Object(ref map) if map.get("result").is_some() => {
            match map.get("result").cloned().unwrap_or(Value::Null) {
                Value::Array(items) => items,
                other => vec![other],
            }
        }
        other => vec![other],
    };

    let mut txs = Vec::new();
    for item in items {
        if item.is_null() {
            continue;
        }
        let facts =
            decode::tx_from_node_json(&item, None, None).map_err(|e| anyhow::anyhow!("{e}"))?;
        txs.push(facts);
    }
    if txs.is_empty() {
        bail!("no transactions found in {}", path.display());
    }
    let first = txs[0].txid.clone();
    let mut subject = Subject::new(
        label
            .map(str::to_string)
            .unwrap_or_else(|| format!("tx {}", short(&first))),
        SubjectKind::Transaction,
        "main",
    );
    subject.txs = txs;
    Ok(subject)
}

fn ci(path: &Path, fail_on: &str) -> Result<()> {
    let threshold = parse_severity(fail_on)?;
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let report: Report = serde_json::from_str(&text)
        .with_context(|| format!("{} is not a LeakBench report", path.display()))?;

    let mut failures = Vec::new();
    if report.fatal_count() > 0 {
        failures.push(format!(
            "{} fatal finding(s): a disclosure the brief would treat as disqualifying",
            report.fatal_count()
        ));
    }
    if let Some(max) = report.max_severity() {
        if max >= threshold {
            failures.push(format!(
                "highest severity is {} (gate is {})",
                max.as_str(),
                threshold.as_str()
            ));
        }
    }

    if failures.is_empty() {
        println!(
            "  pass  {}  score {}/100 ({})  verdict {}",
            report.subject.label,
            report.score.value,
            report.score.grade,
            report.verdict.as_str()
        );
        Ok(())
    } else {
        println!(
            "  fail  {}  score {}/100 ({})  verdict {}",
            report.subject.label,
            report.score.value,
            report.score.grade,
            report.verdict.as_str()
        );
        for f in &failures {
            println!("        - {f}");
        }
        std::process::exit(1);
    }
}

fn parse_severity(s: &str) -> Result<Severity> {
    match s.to_ascii_lowercase().as_str() {
        "critical" => Ok(Severity::Critical),
        "high" => Ok(Severity::High),
        "medium" => Ok(Severity::Medium),
        "low" => Ok(Severity::Low),
        "info" => Ok(Severity::Info),
        other => bail!("unknown severity {other:?} (critical|high|medium|low|info)"),
    }
}

fn doctor(url: &str) -> Result<()> {
    let client = Rpc::parse(url).map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("\n  endpoint  {}", client.endpoint());
    match client.block_count() {
        Ok(height) => {
            println!("  ok        getblockcount -> {height}");
            match client.block_hash(height) {
                Ok(hash) => {
                    println!("  ok        getblockhash -> {}", short(&hash));
                    match client.block(&hash, 1) {
                        Ok(block) => {
                            let txs = block
                                .get("tx")
                                .and_then(|t| t.as_array())
                                .map(|a| a.len())
                                .unwrap_or(0);
                            println!("  ok        getblock(verbosity=1) -> {txs} tx(s)");
                        }
                        Err(e) => println!("  warn      getblock failed: {e}"),
                    }
                }
                Err(e) => println!("  warn      getblockhash failed: {e}"),
            }
            println!(
                "\n  This node can be used with `leakbench scan --rpc {} --txid <txid>`.\n",
                client.endpoint()
            );
            Ok(())
        }
        Err(e) => {
            println!("  fail      {e}");
            println!("\n  Is the node running with `[rpc] listen_addr` set, and is the URL right?");
            println!("  `leakbench devnet` prints a working Zebra Regtest config.\n");
            Ok(())
        }
    }
}

fn devnet_cmd(config: bool) -> Result<()> {
    if config {
        print!("{}", devnet::ZEBRAD_REGTEST_TOML);
        return Ok(());
    }
    println!("\n  LOCAL NETWORK RECIPES\n");
    for (name, steps) in [
        ("Offline (works now, no node)", devnet::offline_recipe()),
        ("Zebra Regtest (local chain)", devnet::regtest_recipe()),
    ] {
        println!("  {}", name);
        for (i, step) in steps.iter().enumerate() {
            println!("    {}. {}", i + 1, step.title);
            if let Some(cmd) = step.command {
                println!("       $ {cmd}");
            }
            println!("       {}", step.note);
        }
        println!();
    }
    println!("  NOTES");
    for note in devnet::verify_notes() {
        println!("   - {note}");
    }
    println!("\n  `leakbench devnet --config > zebrad.toml` writes the node config.\n");
    Ok(())
}

fn short(s: &str) -> String {
    if s.len() <= 16 {
        s.to_string()
    } else {
        format!("{}…", &s[..16])
    }
}
