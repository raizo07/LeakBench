//! `leakbench` — analyze Zcash transactions for privacy leaks.

mod render;
mod sarif;
mod schema;
mod serve;
mod wallet;

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use leakbench_chain::peers::{self, PeerSet};
use leakbench_chain::{decode, devnet, fixtures, rpc::Rpc};
use leakbench_core::facts::PeerSummary;
use leakbench_core::{
    analyze, analyze_at, diff::diff as diff_reports, Report, Severity, Subject, SubjectKind,
    WalletFacts,
};
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
    /// One line per report; for CI logs.
    #[arg(long, global = true)]
    quiet: bool,
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
        /// Pin generated_at (RFC 3339 or epoch seconds) so output is byte-stable.
        #[arg(long)]
        as_of: Option<String>,
    },
    /// Analyze one built-in scenario.
    Simulate {
        #[arg(long, default_value = "naive")]
        scenario: String,
        #[arg(long)]
        out: Option<PathBuf>,
        /// Pin generated_at (RFC 3339 or epoch seconds).
        #[arg(long)]
        as_of: Option<String>,
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
        /// Anonymity-set observations, from `leakbench peers`.
        #[arg(long)]
        peers: Option<PathBuf>,
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Sample an anonymity set from a block range.
    Peers {
        #[arg(long, default_value = "http://127.0.0.1:18232")]
        rpc: String,
        #[arg(long)]
        from: u64,
        #[arg(long)]
        to: u64,
        #[arg(long, default_value = "main")]
        network: String,
        /// Target block spacing: 75 before NU7, 25 after.
        #[arg(long, default_value_t = 75)]
        block_interval: u64,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Compare two reports and flag regressions.
    Diff {
        before: PathBuf,
        after: PathBuf,
        /// Exit non-zero when the score drops or a new fatal finding appears.
        #[arg(long)]
        fail_on_regression: bool,
        #[arg(long)]
        json: bool,
    },
    /// Render a report as SARIF 2.1.0 for code scanning.
    Sarif {
        #[arg(long)]
        report: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Wallet-side facts.
    Wallet {
        #[command(subcommand)]
        cmd: WalletCmd,
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

#[derive(Subcommand)]
enum WalletCmd {
    /// Build wallet.json from CSV exports (see docs/FACTS.md).
    Import {
        /// txid,pool,value_zat,received_height[,memo_len,is_change]
        #[arg(long)]
        notes: PathBuf,
        /// address,uses,first_height,last_height
        #[arg(long)]
        addresses: Option<PathBuf>,
        #[arg(long, default_value = "my wallet")]
        label: String,
        #[arg(long, default_value = "main")]
        network: String,
        #[arg(long, default_value_t = 0)]
        birthday: u64,
        #[arg(long)]
        out: PathBuf,
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
    let quiet = cli.quiet;
    match cli.cmd {
        Cmd::Demo { out, web, as_of } => demo(&out, web.as_deref(), as_of.as_deref()),
        Cmd::Simulate {
            scenario,
            out,
            as_of,
            json,
        } => {
            let sc = leakbench_sim::get(&scenario).ok_or_else(|| {
                anyhow::anyhow!("unknown scenario {scenario:?} (try: naive, hardened)")
            })?;
            let report = analyze_at(&sc.subject, parse_as_of(as_of.as_deref())?);
            emit(&report, out.as_deref(), json, quiet)
        }
        Cmd::Scan {
            fixture,
            decoded,
            rpc,
            txid,
            wallet,
            peers,
            label,
            out,
            json,
        } => scan(
            fixture, decoded, rpc, txid, wallet, peers, label, out, json, quiet,
        ),
        Cmd::Peers {
            rpc,
            from,
            to,
            network,
            block_interval,
            out,
            json,
        } => peers_cmd(
            &rpc,
            from,
            to,
            &network,
            block_interval,
            out.as_deref(),
            json,
        ),
        Cmd::Diff {
            before,
            after,
            fail_on_regression,
            json,
        } => diff_cmd(&before, &after, fail_on_regression, json),
        Cmd::Sarif { report, out } => sarif_cmd(&report, out.as_deref()),
        Cmd::Wallet { cmd } => wallet_cmd(cmd),
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

fn demo(out: &Path, web: Option<&Path>, as_of: Option<&str>) -> Result<()> {
    let at = parse_as_of(as_of)?;
    std::fs::create_dir_all(out).with_context(|| format!("cannot create {}", out.display()))?;
    if let Some(web) = web {
        std::fs::create_dir_all(web).with_context(|| format!("cannot create {}", web.display()))?;
    }

    let mut reports = Vec::new();
    for sc in leakbench_sim::all() {
        let report = analyze_at(&sc.subject, at);
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

fn emit(report: &Report, out: Option<&Path>, json: bool, quiet: bool) -> Result<()> {
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
    } else if quiet {
        println!(
            "{}  score={} grade={} verdict={} findings={} fatal={}",
            report.subject.label,
            report.score.value,
            report.score.grade,
            report.verdict.as_str(),
            report.findings.len(),
            report.fatal_count()
        );
        if let Some(path) = out {
            println!("wrote {}", path.display());
        }
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
    peers: Option<PathBuf>,
    label: Option<String>,
    out: Option<PathBuf>,
    json: bool,
    quiet: bool,
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

    if let Some(path) = peers {
        let (list, interval) = load_peers(&path)?;
        if list.is_empty() {
            eprintln!("warning: {} contains no peers", path.display());
        }
        subject.peers = list;
        if let Some(interval) = interval {
            subject.context.block_interval_secs = interval;
        }
    }

    let report = analyze(&subject);
    emit(&report, out.as_deref(), json, quiet)
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

fn parse_as_of(value: Option<&str>) -> Result<Option<i64>> {
    match value {
        None => Ok(None),
        Some(text) => leakbench_core::time::parse_timestamp(text)
            .map(Some)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "--as-of must be RFC 3339 (2026-10-27T00:00:00Z) or epoch seconds, got {text:?}"
                )
            }),
    }
}

fn read_report(path: &Path) -> Result<Report> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("{} is not a LeakBench report", path.display()))
}

fn load_peers(path: &Path) -> Result<(Vec<PeerSummary>, Option<u64>)> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let value: Value =
        serde_json::from_str(&text).with_context(|| format!("{} is not JSON", path.display()))?;
    if value.get("peers").is_some() {
        let set: PeerSet = serde_json::from_value(value)
            .with_context(|| format!("{} is not a peer set", path.display()))?;
        Ok((set.peers, Some(set.block_interval_secs)))
    } else {
        let peers: Vec<PeerSummary> = serde_json::from_value(value)
            .with_context(|| format!("{} is not a peer list", path.display()))?;
        Ok((peers, None))
    }
}

#[allow(clippy::too_many_arguments)]
fn peers_cmd(
    url: &str,
    from: u64,
    to: u64,
    network: &str,
    block_interval: u64,
    out: Option<&Path>,
    json: bool,
) -> Result<()> {
    let client = Rpc::parse(url).map_err(|e| anyhow::anyhow!("{e}"))?;
    eprintln!(
        "sampling blocks {from}–{to} ({} blocks) from {}",
        to.saturating_sub(from) + 1,
        client.endpoint()
    );
    let set = peers::collect(&client, from, to, network, block_interval)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if let Some(path) = out {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, serde_json::to_string_pretty(&set)?)
            .with_context(|| format!("cannot write {}", path.display()))?;
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&set)?);
    } else {
        print!("{}", render::render_peers(&set));
        if let Some(path) = out {
            println!("  wrote {}", path.display());
        }
        println!(
            "  use    leakbench scan --peers {} --fixture <subject.json>\n",
            out.map(|p| p.display().to_string())
                .unwrap_or_else(|| "peers.json".into())
        );
    }
    Ok(())
}

fn diff_cmd(before: &Path, after: &Path, fail_on_regression: bool, json: bool) -> Result<()> {
    let b = read_report(before)?;
    let a = read_report(after)?;
    let d = diff_reports(&b, &a);
    if json {
        println!("{}", serde_json::to_string_pretty(&d)?);
    } else {
        print!("{}", render::render_diff(&d));
    }
    if fail_on_regression && d.is_regression() {
        std::process::exit(1);
    }
    Ok(())
}

fn sarif_cmd(report: &Path, out: Option<&Path>) -> Result<()> {
    let report = read_report(report)?;
    let sarif = sarif::build(&report);
    let text = serde_json::to_string_pretty(&sarif)?;
    match out {
        Some(path) => {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            std::fs::write(path, text)
                .with_context(|| format!("cannot write {}", path.display()))?;
            println!(
                "  wrote {} ({} results)",
                path.display(),
                report.findings.len()
            );
        }
        None => println!("{text}"),
    }
    Ok(())
}

fn wallet_cmd(cmd: WalletCmd) -> Result<()> {
    match cmd {
        WalletCmd::Import {
            notes,
            addresses,
            label,
            network,
            birthday,
            out,
        } => {
            let notes_text = std::fs::read_to_string(&notes)
                .with_context(|| format!("cannot read {}", notes.display()))?;
            let notes = wallet::parse_notes(&notes_text)?;
            let addresses = match addresses {
                Some(path) => {
                    let text = std::fs::read_to_string(&path)
                        .with_context(|| format!("cannot read {}", path.display()))?;
                    wallet::parse_addresses(&text)?
                }
                None => Vec::new(),
            };
            let facts = wallet::build(&label, &network, birthday, notes, addresses);
            if let Some(parent) = out.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            std::fs::write(&out, serde_json::to_string_pretty(&facts)?)
                .with_context(|| format!("cannot write {}", out.display()))?;
            println!(
                "  wrote {} ({} notes, {} transparent addresses)",
                out.display(),
                facts.notes.len(),
                facts.addresses.len()
            );
            println!(
                "  next  leakbench scan --fixture <subject.json> --wallet {}",
                out.display()
            );
            Ok(())
        }
    }
}

fn short(s: &str) -> String {
    if s.len() <= 16 {
        s.to_string()
    } else {
        format!("{}…", &s[..16])
    }
}
