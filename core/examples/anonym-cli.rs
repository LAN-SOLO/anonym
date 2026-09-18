//! Kleiner Kommandozeilen-Lauf über den Kern:
//! `cargo run --example anonym-cli -- <datei> [--apply] [--export=xlsx,pdf,…]`
//! Druckt die Fundstellen, schreibt bei `--apply` `<name>.anonym.<ext>` daneben und
//! bei `--export` zusätzlich die genannten Formate.

use anonym_core::{analyze, apply_file, suggest_output, suggest_output_as, Dictionaries, Options, Rules};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let apply = args.iter().any(|a| a == "--apply");
    let exports: Vec<String> = args.iter().filter_map(|a| a.strip_prefix("--export=")).flat_map(|v| v.split(',').map(|s| s.trim().to_string())).collect();
    let files: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let dicts = Dictionaries::builtin();
    let rules = Rules::default();
    let opts = Options::default();
    for f in files {
        let path = Path::new(f);
        match analyze(path, &rules, &dicts, &opts) {
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
