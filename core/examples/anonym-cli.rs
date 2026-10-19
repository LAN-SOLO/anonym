//! Kleiner Kommandozeilen-Lauf über den Kern:
//! `cargo run --example anonym-cli -- <datei> [--apply] [--export=xlsx,pdf,…] [--rules=regeln.json] [--json]`
//! Druckt die Fundstellen, schreibt bei `--apply` `<name>.anonym.<ext>` daneben und
//! bei `--export` zusätzlich die genannten Formate. `--json` druckt die Analyse als JSON
//! (wie sie die App bekommt), `--rules` liest ein exportiertes Regelwerk.

use anonym_core::{analyze, apply_file, suggest_output, suggest_output_as, Dictionaries, Options, Rules};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let apply = args.iter().any(|a| a == "--apply");
    let exports: Vec<String> = args.iter().filter_map(|a| a.strip_prefix("--export=")).flat_map(|v| v.split(',').map(|s| s.trim().to_string())).collect();
    let files: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let json = args.iter().any(|a| a == "--json");
    let dicts = Dictionaries::builtin();
    let rules: Rules = match args.iter().find_map(|a| a.strip_prefix("--rules=")) {
        Some(p) => serde_json::from_str(&std::fs::read_to_string(p).expect("Regelwerk nicht lesbar")).expect("Kein gültiges Regelwerk"),
        None => Rules::default(),
    };
    let opts = Options::default();
    for f in files {
        let path = Path::new(f);
        match analyze(path, &rules, &dicts, &opts) {
            Ok(a) if json => println!("{}", serde_json::to_string(&a).unwrap()),
            Ok(a) => {
                println!("== {} [{} · {} · {} Zeilen · {} Fundstellen]", a.path, a.format, a.encoding, a.lines, a.findings.len());
                for n in &a.notes {
                    println!("   // {n}");
                }
                for x in &a.findings {
                    println!("   {:>4} {:<12} {:<9} {:>3}%  {:<32} → {}", x.line, x.category.key(), format!("{:?}", x.source).to_lowercase(), x.confidence, x.text, x.replacement);
                }
                if apply {
                    let out = suggest_output(path, None, "");
                    match apply_file(path, &rules, &dicts, &opts, &[], &out) {
                        Ok(r) => println!("   → {} ({} Ersetzungen)", r.output.display(), r.report.replaced),
                        Err(e) => println!("   FEHLER: {e}"),
                    }
                }
                for ext in &exports {
                    let out = suggest_output_as(path, None, "", Some(ext));
                    match apply_file(path, &rules, &dicts, &opts, &[], &out) {
                        Ok(r) => println!("   → {} [{}]", r.output.display(), r.report.format),
                        Err(e) => println!("   FEHLER ({ext}): {e}"),
                    }
                }
            }
            Err(e) => println!("== {f}: FEHLER {e}"),
        }
    }
}
