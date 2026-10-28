//! Generates the `typst-codex` Espanso package from Typst's `codex` crate.
//!
//! - `generator` regenerates `typst-codex/package.yml` and records the codex
//!   version in the manifest and README.
//! - `generator release <auto|patch|minor|major> <last-release-dir>` does the
//!   same, then bumps the manifest version against the last released package
//!   and prints a Markdown summary of the symbol changes. In GitHub Actions,
//!   the version goes to `$GITHUB_OUTPUT` (unset if the package is unchanged)
//!   and the summary to `$GITHUB_STEP_SUMMARY`.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Write as _;
use std::path::Path;

use codex::{Def, Module};

const PACKAGE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../typst-codex");
const CARGO_LOCK: &str = include_str!("../Cargo.lock");
const SOURCE_PREFIX: &str = "Symbols from codex ";

/// A single symbol: its full dotted Typst name and its value.
#[derive(Debug)]
struct Entry {
    name: String,
    value: &'static str,
}

/// Walks `module` recursively, appending every non-deprecated variant in
/// codex's iteration order.
fn collect(module: Module, prefix: &str, out: &mut Vec<Entry>) {
    for (name, binding) in module.iter() {
        if binding.deprecation.is_some() {
            continue;
        }
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}.{name}")
        };
        match binding.def {
            Def::Module(sub) => collect(sub, &path, out),
            Def::Symbol(symbol) => {
                for (modifiers, value, deprecation) in symbol.variants() {
                    if deprecation.is_some() {
                        continue;
                    }
                    let name = if modifiers.is_empty() {
                        path.clone()
                    } else {
                        format!("{path}.{}", modifiers.as_str())
                    };
                    out.push(Entry { name, value });
                }
            }
        }
    }
}

/// All entries: `sym` unprefixed, then `emoji` prefixed with `emoji`.
fn entries() -> Vec<Entry> {
    let mut out = Vec::new();
    collect(codex::SYM, "", &mut out);
    collect(codex::EMOJI, "emoji", &mut out);
    out
}

fn trigger(name: &str) -> String {
    format!(";{name};")
}

/// Panics if two entries would produce the same trigger.
fn assert_unique(entries: &[Entry]) {
    let mut seen = HashSet::new();
    for entry in entries {
        assert!(
            seen.insert(entry.name.as_str()),
            "duplicate trigger: {}",
            trigger(&entry.name)
        );
    }
}

fn quote(s: &str) -> String {
    serde_json::to_string(s).expect("strings always serialize")
}

fn render(entries: &[Entry]) -> String {
    let mut out =
        String::from("# Generated from the `codex` crate. Do not edit by hand.\nmatches:\n");
    for Entry { name, value } in entries {
        out.push_str(&format!("  - trigger: {}\n", quote(&trigger(name))));
        out.push_str(&format!("    replace: {}\n", quote(value)));
        out.push_str(&format!(
            "    label: {}\n",
            quote(&format!("{value}  {name}"))
        ));
    }
    out
}

/// Reads `trigger -> replace` back out of a rendered package.
fn symbols(package: &str) -> BTreeMap<String, String> {
    let field = |line: &str, key: &str| -> String {
        let quoted = line
            .trim_start()
            .trim_start_matches("- ")
            .strip_prefix(key)
            .expect("rendered format");
        serde_json::from_str(quoted).expect("values are JSON strings")
    };
    let mut lines = package
        .lines()
        .skip_while(|line| !line.starts_with("  - trigger: "));
    let mut out = BTreeMap::new();
    while let (Some(trigger), Some(replace), Some(_label)) =
        (lines.next(), lines.next(), lines.next())
    {
        out.insert(field(trigger, "trigger: "), field(replace, "replace: "));
    }
    out
}

/// The locked codex version.
fn codex_version(lock: &str) -> &str {
    lock.split("[[package]]")
        .find_map(|pkg| pkg.trim().strip_prefix("name = \"codex\"\nversion = \""))
        .and_then(|rest| rest.split('"').next())
        .expect("codex is in Cargo.lock")
}

/// The codex version recorded after `SOURCE_PREFIX`, e.g. "0.3.0".
fn recorded_codex(text: &str) -> Option<&str> {
    let rest = &text[text.find(SOURCE_PREFIX)? + SOURCE_PREFIX.len()..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(rest.len());
    Some(rest[..end].trim_end_matches('.'))
}

/// Replaces every recorded codex version in `text` with `version`.
fn set_codex(text: &str, version: &str) -> String {
    match recorded_codex(text) {
        Some(old) => text.replace(
            &format!("{SOURCE_PREFIX}{old}"),
            &format!("{SOURCE_PREFIX}{version}"),
        ),
        None => text.to_string(),
    }
}

/// The value of a top-level `key: value` line in the manifest.
fn manifest_field<'a>(manifest: &'a str, key: &str) -> &'a str {
    manifest
        .lines()
        .find_map(|line| line.strip_prefix(key)?.strip_prefix(": "))
        .unwrap_or_else(|| panic!("manifest has no `{key}` line"))
        .trim()
}

fn set_manifest_field(manifest: &str, key: &str, value: &str) -> String {
    let old = format!("{key}: {}", manifest_field(manifest, key));
    manifest.replacen(&old, &format!("{key}: {value}"), 1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bump {
    Patch,
    Minor,
    Major,
}

fn bump_version(version: &str, bump: Bump) -> String {
    let parts: Vec<u64> = version
        .split('.')
        .map(|p| p.parse().expect("semver version"))
        .collect();
    let [major, minor, patch] = parts[..] else {
        panic!("not a semver version: {version}")
    };
    match bump {
        Bump::Major => format!("{}.0.0", major + 1),
        Bump::Minor => format!("{major}.{}.0", minor + 1),
        Bump::Patch => format!("{major}.{minor}.{}", patch + 1),
    }
}

/// A pending release and what changed since the last one.
#[derive(Debug, PartialEq)]
struct Plan {
    bump: Bump,
    version: String,
    added: Vec<(String, String)>,
    removed: Vec<(String, String)>,
    /// `(trigger, old, new)` for triggers whose replacement changed.
    changed: Vec<(String, String, String)>,
}

/// Decides the next release given the last released package and manifest.
/// Returns `None` if the package didn't change.
fn plan(
    old_package: &str,
    old_manifest: &str,
    new_package: &str,
    codex: &str,
    bump: &str,
) -> Result<Option<Plan>, String> {
    if old_package == new_package {
        return Ok(None);
    }
    let bump = match bump {
        "patch" => Bump::Patch,
        "minor" => Bump::Minor,
        "major" => Bump::Major,
        "auto" if recorded_codex(old_manifest) == Some(codex) => Bump::Patch,
        "auto" => Bump::Minor,
        other => return Err(format!("invalid bump: {other}")),
    };
    let (old, new) = (symbols(old_package), symbols(new_package));
    let only_in =
        |a: &BTreeMap<String, String>, b: &BTreeMap<String, String>| -> Vec<(String, String)> {
            a.iter()
                .filter(|(t, _)| !b.contains_key(*t))
                .map(|(t, v)| (t.clone(), v.clone()))
                .collect()
        };
    let removed = only_in(&old, &new);
    if !removed.is_empty() && bump != Bump::Major {
        let names: Vec<_> = removed.iter().map(|(t, _)| t.as_str()).collect();
        return Err(format!(
            "{} trigger(s) removed: {}\nRemoving triggers is a breaking change. \
             Re-run the release workflow with bump=major if this is intended.",
            removed.len(),
            names.join(" ")
        ));
    }
    let changed = new
        .iter()
        .filter_map(|(t, v)| {
            old.get(t)
                .filter(|o| *o != v)
                .map(|o| (t.clone(), o.clone(), v.clone()))
        })
        .collect();
    Ok(Some(Plan {
        bump,
        version: bump_version(manifest_field(old_manifest, "version"), bump),
        added: only_in(&new, &old),
        removed,
        changed,
    }))
}

/// Markdown summary of a plan, shown on the workflow run before approval.
fn summary(plan: &Plan, old_codex: &str, codex: &str) -> String {
    let mut out = format!(
        "## typst-codex v{} ({:?} release)\n\n",
        plan.version, plan.bump
    );
    out += &format!("Symbols from codex {codex} (last release: codex {old_codex}).\n\n");
    out += &format!(
        "**{} added, {} removed, {} changed.**\n",
        plan.added.len(),
        plan.removed.len(),
        plan.changed.len()
    );
    if plan.added.is_empty() && plan.removed.is_empty() && plan.changed.is_empty() {
        out += "\nNo symbol changes, only the output format (e.g. labels) changed.\n";
    }
    let mut section = |title: &str, rows: Vec<String>| {
        if !rows.is_empty() {
            out += &format!(
                "\n<details><summary>{title} ({})</summary>\n\n{}\n</details>\n",
                rows.len(),
                rows.join("\n")
            );
        }
    };
    section(
        "Added",
        plan.added
            .iter()
            .map(|(t, v)| format!("- `{t}` {v}"))
            .collect(),
    );
    section(
        "Removed",
        plan.removed
            .iter()
            .map(|(t, v)| format!("- `{t}` {v}"))
            .collect(),
    );
    section(
        "Changed",
        plan.changed
            .iter()
            .map(|(t, o, n)| format!("- `{t}` {o} → {n}"))
            .collect(),
    );
    out
}

/// Appends to the file named by a GitHub Actions env var, if it's set.
fn append_github_file(var: &str, contents: &str) {
    if let Some(path) = std::env::var_os(var) {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .expect("GitHub file is writable");
        file.write_all(contents.as_bytes())
            .expect("GitHub file is writable");
    }
}

fn read(path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

fn write(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    fs::write(path, contents).unwrap_or_else(|e| panic!("failed to write {}: {e}", path.display()));
}

/// Regenerates the package in `dir` and returns the new `package.yml`.
fn generate(dir: &Path) -> String {
    let entries = entries();
    assert_unique(&entries);
    let package = render(&entries);
    let codex = codex_version(CARGO_LOCK);
    write(dir.join("package.yml"), &package);
    for file in ["_manifest.yml", "README.md"] {
        write(dir.join(file), &set_codex(&read(dir.join(file)), codex));
    }
    eprintln!("wrote {} matches from codex {codex}", entries.len());
    package
}

fn main() {
    let dir = Path::new(PACKAGE_DIR);
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => {
            generate(dir);
        }
        [cmd, bump, last] if cmd == "release" => {
            let last = Path::new(last);
            let package = generate(dir);
            let codex = codex_version(CARGO_LOCK);
            let old_manifest = read(last.join("_manifest.yml"));
            match plan(
                &read(last.join("package.yml")),
                &old_manifest,
                &package,
                codex,
                bump,
            ) {
                Ok(Some(plan)) => {
                    let manifest = read(dir.join("_manifest.yml"));
                    write(
                        dir.join("_manifest.yml"),
                        &set_manifest_field(&manifest, "version", &plan.version),
                    );
                    let summary = summary(
                        &plan,
                        recorded_codex(&old_manifest).unwrap_or("unknown"),
                        codex,
                    );
                    eprintln!("{summary}");
                    append_github_file("GITHUB_STEP_SUMMARY", &summary);
                    append_github_file("GITHUB_OUTPUT", &format!("version={}\n", plan.version));
                }
                Ok(None) => {
                    eprintln!("package unchanged, nothing to release");
                    append_github_file(
                        "GITHUB_STEP_SUMMARY",
                        "Package unchanged since the last release, nothing to release.\n",
                    );
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                }
            }
        }
        _ => {
            eprintln!("usage: generator [release <auto|patch|minor|major> <last-release-dir>]");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yaml_rust2::YamlLoader;

    const MANIFEST: &str = "name: typst-codex\n\
        description: Typst symbol names as Espanso triggers. Symbols from codex 0.3.0.\n\
        version: 1.4.2\n";

    fn entry(name: &str, value: &'static str) -> Entry {
        Entry {
            name: name.into(),
            value,
        }
    }

    fn package(names: &[&str]) -> String {
        render(&names.iter().map(|n| entry(n, "x")).collect::<Vec<_>>())
    }

    fn value(entries: &[Entry], name: &str) -> Option<&'static str> {
        entries.iter().find(|e| e.name == name).map(|e| e.value)
    }

    #[test]
    fn known_symbols() {
        let entries = entries();
        assert_eq!(value(&entries, "alpha"), Some("α"));
        assert_eq!(value(&entries, "arrow.r"), Some("→"));
        assert_eq!(value(&entries, "arrow.r.double"), Some("⇒"));
        assert_eq!(value(&entries, "emoji.rocket"), Some("🚀"));
    }

    #[test]
    fn skips_deprecated() {
        // `gt.tri` is a deprecated variant in codex 0.3.0, replaced by `gt.closed`.
        let entries = entries();
        assert!(value(&entries, "gt.closed").is_some());
        assert!(value(&entries, "gt.tri").is_none());
    }

    #[test]
    fn real_entries_are_unique() {
        assert_unique(&entries());
    }

    #[test]
    #[should_panic(expected = "duplicate trigger: ;a;")]
    fn duplicates_panic() {
        assert_unique(&[entry("a", "x"), entry("a", "y")]);
    }

    #[test]
    fn output_parses_as_yaml() {
        let entries = entries();
        let docs = YamlLoader::load_from_str(&render(&entries)).unwrap();
        let matches = docs[0]["matches"].as_vec().unwrap();
        assert!(
            (2000..4000).contains(&matches.len()),
            "{} matches",
            matches.len()
        );
        assert_eq!(matches.len(), entries.len());
        for (m, e) in matches.iter().zip(&entries) {
            assert_eq!(m["trigger"].as_str(), Some(trigger(&e.name).as_str()));
            assert_eq!(m["replace"].as_str(), Some(e.value));
            assert_eq!(
                m["label"].as_str(),
                Some(format!("{}  {}", e.value, e.name).as_str())
            );
        }
    }

    #[test]
    fn symbols_round_trip() {
        let entries = entries();
        let symbols = symbols(&render(&entries));
        assert_eq!(symbols.len(), entries.len());
        assert!(
            entries
                .iter()
                .all(|e| symbols[&trigger(&e.name)] == e.value)
        );
    }

    #[test]
    fn package_files_are_releasable() {
        let text = read(Path::new(PACKAGE_DIR).join("_manifest.yml"));
        let manifest = &YamlLoader::load_from_str(&text).unwrap()[0];
        // Fields required by the Hub's CI.
        for key in ["name", "title", "description", "version", "author"] {
            assert!(
                manifest[key].as_str().is_some(),
                "manifest is missing `{key}`"
            );
        }
        assert_eq!(manifest["name"].as_str(), Some("typst-codex"));
        bump_version(manifest["version"].as_str().unwrap(), Bump::Patch);
        bump_version(codex_version(CARGO_LOCK), Bump::Patch);
        assert!(recorded_codex(&text).is_some());
        assert!(recorded_codex(&read(Path::new(PACKAGE_DIR).join("README.md"))).is_some());
    }

    #[test]
    fn codex_version_in_text() {
        assert_eq!(recorded_codex(MANIFEST), Some("0.3.0"));
        assert_eq!(recorded_codex("Symbols from codex 0.10.2"), Some("0.10.2"));
        assert!(set_codex(MANIFEST, "0.4.0").contains("Symbols from codex 0.4.0.\n"));
    }

    #[test]
    fn manifest_fields() {
        let text = set_manifest_field(MANIFEST, "version", "2.0.0");
        assert_eq!(manifest_field(&text, "version"), "2.0.0");
        assert_eq!(manifest_field(&text, "name"), "typst-codex");
    }

    #[test]
    fn bump_versions() {
        assert_eq!(bump_version("1.4.2", Bump::Patch), "1.4.3");
        assert_eq!(bump_version("1.4.2", Bump::Minor), "1.5.0");
        assert_eq!(bump_version("1.4.2", Bump::Major), "2.0.0");
    }

    #[test]
    fn plan_unchanged() {
        let p = package(&["a"]);
        assert_eq!(plan(&p, MANIFEST, &p, "0.4.0", "major"), Ok(None));
    }

    #[test]
    fn plan_bump() {
        let (old, new) = (package(&["a"]), package(&["a", "b"]));
        let bump = |codex, input| {
            let p = plan(&old, MANIFEST, &new, codex, input).unwrap().unwrap();
            (p.bump, p.version)
        };
        assert_eq!(bump("0.3.0", "auto"), (Bump::Patch, "1.4.3".into()));
        assert_eq!(bump("0.4.0", "auto"), (Bump::Minor, "1.5.0".into()));
        assert_eq!(bump("0.4.0", "major"), (Bump::Major, "2.0.0".into()));
        assert!(plan(&old, MANIFEST, &new, "0.3.0", "huge").is_err());
    }

    #[test]
    fn plan_removed_triggers_need_major() {
        let (old, new) = (package(&["a", "b"]), package(&["a"]));
        let err = plan(&old, MANIFEST, &new, "0.4.0", "auto").unwrap_err();
        assert!(err.contains(";b;") && err.contains("bump=major"), "{err}");
        let plan = plan(&old, MANIFEST, &new, "0.4.0", "major")
            .unwrap()
            .unwrap();
        assert_eq!(plan.removed, vec![(";b;".into(), "x".into())]);
    }

    #[test]
    fn plan_summary() {
        let old = render(&[entry("a", "x"), entry("b", "y")]);
        let new = render(&[entry("a", "z"), entry("b", "y"), entry("c", "w")]);
        let plan = plan(&old, MANIFEST, &new, "0.4.0", "auto")
            .unwrap()
            .unwrap();
        assert_eq!(plan.added, vec![(";c;".into(), "w".into())]);
        assert_eq!(plan.changed, vec![(";a;".into(), "x".into(), "z".into())]);

        let text = summary(&plan, "0.3.0", "0.4.0");
        assert!(
            text.starts_with("## typst-codex v1.5.0 (Minor release)\n"),
            "{text}"
        );
        assert!(
            text.contains("**1 added, 0 removed, 1 changed.**"),
            "{text}"
        );
        assert!(
            text.contains("- `;c;` w") && text.contains("- `;a;` x → z"),
            "{text}"
        );
        assert!(!text.contains("Removed"), "{text}");
    }

    #[test]
    fn label_only_change_is_patch() {
        let old = package(&["a"]).replace("label: \"x  a\"", "label: \"old\"");
        let plan = plan(&old, MANIFEST, &package(&["a"]), "0.3.0", "auto")
            .unwrap()
            .unwrap();
        assert_eq!(plan.bump, Bump::Patch);
        assert!(summary(&plan, "0.3.0", "0.3.0").contains("only the output format"));
    }
}
