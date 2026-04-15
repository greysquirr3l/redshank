//! `WikiGraphModel` — petgraph DAG of wiki entries and cross-references.
//!
//! Parses `wiki/index.md`, reads individual entry files to extract bold
//! cross-references, fuzzy-matches entity names across a name registry,
//! and builds a `petgraph::DiGraph`.

#[cfg(feature = "runtime")]
use chrono::{DateTime, Utc};
#[cfg(feature = "runtime")]
use std::collections::HashMap;
#[cfg(feature = "runtime")]
use std::path::{Path, PathBuf};
#[cfg(feature = "runtime")]
use std::sync::Arc;

#[cfg(feature = "runtime")]
use petgraph::graph::{DiGraph, NodeIndex};
#[cfg(feature = "runtime")]
use petgraph::visit::EdgeRef;
#[cfg(feature = "runtime")]
use regex::Regex;
#[cfg(feature = "runtime")]
use tokio::sync::RwLock;

#[cfg(feature = "runtime")]
use crate::domain::wiki::{WikiCategory, WikiEntry};

// ── Category colours (for TUI rendering) ────────────────────────────────────

#[cfg(feature = "runtime")]
pub const CATEGORY_COLORS: &[(WikiCategory, &str)] = &[
    (WikiCategory::CampaignFinance, "Cyan"),
    (WikiCategory::Contracts, "Yellow"),
    (WikiCategory::Corporate, "Green"),
    (WikiCategory::Financial, "Red"),
    (WikiCategory::International, "Magenta"),
    (WikiCategory::Lobbying, "Blue"),
    (WikiCategory::Nonprofits, "White"),
    (WikiCategory::Infrastructure, "BrightGreen"),
    (WikiCategory::People, "BrightCyan"),
    (WikiCategory::Other, "Gray"),
];

/// Return the TUI colour for a wiki category.
#[cfg(feature = "runtime")]
#[must_use]
pub fn category_color(cat: &WikiCategory) -> &'static str {
    CATEGORY_COLORS
        .iter()
        .find(|(c, _)| c == cat)
        .map_or("Gray", |(_, color)| *color)
}

// ── Node / Edge types ───────────────────────────────────────────────────────

#[cfg(feature = "runtime")]
#[derive(Debug, Clone)]
pub struct WikiNode {
    pub name: String,
    pub category: WikiCategory,
    pub title: String,
    pub rel_path: PathBuf,
    pub first_seen: Option<DateTime<Utc>>,
    pub last_seen: Option<DateTime<Utc>>,
    pub observation_count: u32,
}

#[cfg(feature = "runtime")]
#[derive(Debug, Clone)]
pub struct WikiEdge {
    pub ref_text: String,
    pub first_seen: Option<DateTime<Utc>>,
    pub last_seen: Option<DateTime<Utc>>,
    pub evidence_sources: Vec<String>,
}

// ── Index parsing ───────────────────────────────────────────────────────────

#[cfg(feature = "runtime")]
fn category_slug(display_name: &str) -> String {
    display_name
        .trim()
        .to_lowercase()
        .replace(" & ", "-")
        .replace(' ', "-")
}

#[cfg(feature = "runtime")]
fn slug_to_category(slug: &str) -> WikiCategory {
    match slug {
        "campaign-finance" => WikiCategory::CampaignFinance,
        "government-contracts" | "contracts" => WikiCategory::Contracts,
        "corporate-registries" | "corporate" => WikiCategory::Corporate,
        "financial" => WikiCategory::Financial,
        "infrastructure" => WikiCategory::Infrastructure,
        "international" => WikiCategory::International,
        "lobbying" => WikiCategory::Lobbying,
        "nonprofits" => WikiCategory::Nonprofits,
        "people" => WikiCategory::People,
        _ => WikiCategory::Other,
    }
}

/// Parse `wiki/index.md` and return `(category, name, rel_path)` triples.
///
/// # Panics
///
/// Panics if the hard-coded regex patterns fail to compile (they are
/// validated at compile time and cannot fail in practice).
#[cfg(feature = "runtime")]
#[must_use]
pub fn parse_index(wiki_dir: &Path) -> Vec<(WikiCategory, String, PathBuf)> {
    let index_path = wiki_dir.join("index.md");
    let Ok(text) = std::fs::read_to_string(&index_path) else {
        return Vec::new();
    };

    let category_re = Regex::new(r"^###\s+(.+)$")
        .unwrap_or_else(|e| unreachable!("regex literal is always valid: {e}"));
    let row_re = Regex::new(
        r"^\|\s*(?P<name>[^|]+?)\s*\|\s*[^|]*?\s*\|\s*\[(?P<link_text>[^\]]+)\]\((?P<path>[^)]+)\)\s*\|",
    )
    .unwrap_or_else(|e| unreachable!("regex literal is always valid: {e}"));

    let mut entries = Vec::new();
    let mut current_category = WikiCategory::Other;

    for line in text.lines() {
        if let Some(caps) = category_re.captures(line) {
            let slug = category_slug(&caps[1]);
            current_category = slug_to_category(&slug);
            continue;
        }
        if let Some(caps) = row_re.captures(line) {
            let name = caps
                .name("name")
                .map_or("", |m| m.as_str())
                .trim()
                .to_owned();
            let path = caps.name("path").map_or("", |m| m.as_str()).trim();
            entries.push((current_category.clone(), name, PathBuf::from(path)));
        }
    }

    entries
}

// ── Cross-reference extraction ──────────────────────────────────────────────

/// Extract cross-references from a wiki file.
///
/// Returns `(title, cross_ref_names)`.
///
/// # Panics
///
/// Panics if the hard-coded regex pattern fails to compile (it is validated
/// and cannot fail in practice).
#[cfg(feature = "runtime")]
#[must_use]
pub fn extract_cross_refs(file_path: &Path) -> (String, Vec<String>) {
    let Ok(text) = std::fs::read_to_string(file_path) else {
        return (String::new(), Vec::new());
    };
    let lines: Vec<&str> = text.lines().collect();

    // Extract title from first `# ` heading.
    let title = lines
        .iter()
        .find(|l| l.starts_with("# ") && !l.starts_with("## "))
        .map(|l| l[2..].trim().to_owned())
        .unwrap_or_default();

    // Find `## Cross-Reference Potential` section.
    let bold_re = Regex::new(r"\*\*([^*]+)\*\*")
        .unwrap_or_else(|e| unreachable!("regex literal is always valid: {e}"));
    let skip_prefixes = ["join", "critical", "geographic"];

    let mut in_section = false;
    let mut refs = Vec::new();

    for line in &lines {
        if line.starts_with("## Cross-Reference Potential") {
            in_section = true;
            continue;
        }
        if in_section && line.starts_with("## ") {
            break;
        }
        if !in_section {
            continue;
        }
        let trimmed = line.trim();
        if !trimmed.starts_with('-') && !trimmed.starts_with('*') {
            continue;
        }
        for caps in bold_re.captures_iter(trimmed) {
            let ref_text = caps[1].trim().to_owned();
            let lower = ref_text.to_lowercase();
            if skip_prefixes.iter().any(|p| lower.starts_with(p)) {
                continue;
            }
            refs.push(ref_text);
        }
    }

    (title, refs)
}

// ── Name registry and fuzzy matching ────────────────────────────────────────

#[cfg(feature = "runtime")]
#[must_use]
pub fn build_name_registry(
    entries: &[(WikiCategory, String, PathBuf, String)],
) -> HashMap<String, NodeIndex<u32>> {
    // Placeholder — populated during graph build.
    // The actual registry maps lowered name variants → NodeIndex.
    let _ = entries;
    HashMap::new()
}

/// Populate the registry from entries and their node indices.
#[cfg(feature = "runtime")]
fn populate_registry(
    entries: &[(String, String, PathBuf, NodeIndex)],
) -> HashMap<String, NodeIndex> {
    let mut registry = HashMap::new();

    for (name, title, rel_path, idx) in entries {
        // Full name.
        registry.insert(name.to_lowercase(), *idx);

        // Title (if different).
        if !title.is_empty() && title.to_lowercase() != name.to_lowercase() {
            registry.insert(title.to_lowercase(), *idx);
        }

        // Parenthetical aliases: "Senate Lobbying Disclosures (LD-1/LD-2)" → "ld-1/ld-2".
        if let Some(start) = name.find('(')
            && let Some(end) = name.find(')')
        {
            let inner = &name[start + 1..end];
            registry.insert(inner.to_lowercase(), *idx);
            let without = name[..start].trim();
            if !without.is_empty() {
                registry.insert(without.to_lowercase(), *idx);
            }
        }

        // Slash-split parts: "ProPublica Nonprofit Explorer / IRS 990".
        if name.contains(" / ") {
            for part in name.split(" / ") {
                let part = part.trim();
                if !part.is_empty() {
                    registry.insert(part.to_lowercase(), *idx);
                }
            }
        }

        // File slug: "campaign-finance/fec-federal.md" → "fec-federal".
        let slug = rel_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !slug.is_empty() {
            registry.insert(slug, *idx);
        }
    }

    registry
}

/// Fuzzy-match a cross-reference mention against the name registry.
///
/// Returns the `NodeIndex` if a match is found.
#[cfg(feature = "runtime")]
#[must_use]
pub fn match_reference<S: std::hash::BuildHasher>(
    ref_text: &str,
    registry: &HashMap<String, NodeIndex, S>,
) -> Option<NodeIndex> {
    let lower = ref_text.to_lowercase();

    // 1. Exact match.
    if let Some(idx) = registry.get(&lower) {
        return Some(*idx);
    }

    // 2. Strip parenthetical from ref and try again.
    if let Some(start) = ref_text.find('(')
        && let Some(end) = ref_text.find(')')
    {
        let inner = ref_text[start + 1..end].to_lowercase();
        if let Some(idx) = registry.get(&inner) {
            return Some(*idx);
        }
        let without = ref_text[..start].trim().to_lowercase();
        if let Some(idx) = registry.get(&without) {
            return Some(*idx);
        }
    }

    // 3. Substring containment.
    for (key, idx) in registry {
        if lower.contains(key.as_str()) || key.contains(lower.as_str()) {
            return Some(*idx);
        }
    }

    // 4. Token overlap (≥2 significant tokens).
    let generic: &[&str] = &[
        "the", "and", "for", "with", "from", "data", "state", "local", "federal",
    ];
    let ref_tokens: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 3 && !generic.contains(t))
        .collect();

    if ref_tokens.len() >= 2 {
        let mut best: Option<NodeIndex> = None;
        let mut best_overlap = 0usize;
        for (key, idx) in registry {
            let key_tokens: Vec<&str> = key
                .split(|c: char| !c.is_alphanumeric())
                .filter(|t| t.len() >= 3 && !generic.contains(t))
                .collect();
            let overlap = ref_tokens.iter().filter(|t| key_tokens.contains(t)).count();
            if overlap > best_overlap && overlap >= 2 {
                best_overlap = overlap;
                best = Some(*idx);
            }
        }
        if best.is_some() {
            return best;
        }
    }

    // 5. Jaro-Winkler similarity (threshold 0.88).
    let mut best_score = 0.0f64;
    let mut best_idx: Option<NodeIndex> = None;
    for (key, idx) in registry {
        let score = jaro_winkler(&lower, key);
        if score > best_score {
            best_score = score;
            best_idx = Some(*idx);
        }
    }
    if best_score >= 0.88 {
        return best_idx;
    }

    None
}

/// Jaro-Winkler similarity score (0.0–1.0).
#[cfg(feature = "runtime")]
fn jaro_winkler(a: &str, b: &str) -> f64 {
    let jaro = jaro(a, b);
    // Winkler bonus: up to 4 common prefix characters.
    let prefix_len = a
        .chars()
        .zip(b.chars())
        .take(4)
        .take_while(|(ca, cb)| ca == cb)
        .count();
    let prefix_f64 = f64::from(u32::try_from(prefix_len).unwrap_or(4));
    f64::mul_add(prefix_f64 * 0.1, 1.0 - jaro, jaro)
}

/// Jaro similarity score.
#[cfg(feature = "runtime")]
#[allow(clippy::indexing_slicing)] // bounds guaranteed by loop invariants on a_len/b_len
fn jaro(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    let match_distance = (a_len.max(b_len) / 2).saturating_sub(1);

    let mut a_matched = vec![false; a_len];
    let mut b_matched = vec![false; b_len];

    let mut matches = 0u32;
    let mut transpositions = 0u32;

    // Find matches.
    for i in 0..a_len {
        let start = i.saturating_sub(match_distance);
        let end = (i + match_distance + 1).min(b_len);
        for j in start..end {
            if b_matched[j] || a_chars[i] != b_chars[j] {
                continue;
            }
            a_matched[i] = true;
            b_matched[j] = true;
            matches += 1;
            break;
        }
    }

    if matches == 0 {
        return 0.0;
    }

    // Count transpositions.
    let mut k = 0usize;
    for i in 0..a_len {
        if !a_matched[i] {
            continue;
        }
        while !b_matched[k] {
            k += 1;
        }
        if a_chars[i] != b_chars[k] {
            transpositions += 1;
        }
        k += 1;
    }

    let m = f64::from(matches);
    (m / f64::from(u32::try_from(a_len).unwrap_or(u32::MAX))
        + m / f64::from(u32::try_from(b_len).unwrap_or(u32::MAX))
        + (m - f64::from(transpositions) / 2.0) / m)
        / 3.0
}

// ── WikiGraphModel ──────────────────────────────────────────────────────────

#[cfg(feature = "runtime")]
pub struct WikiGraphModel {
    wiki_dir: PathBuf,
    graph: DiGraph<WikiNode, WikiEdge>,
    name_registry: HashMap<String, NodeIndex>,
    node_set: HashMap<String, NodeIndex>,
}

#[cfg(feature = "runtime")]
impl WikiGraphModel {
    /// Create a new model for the given wiki directory.
    pub fn new(wiki_dir: impl Into<PathBuf>) -> Self {
        Self {
            wiki_dir: wiki_dir.into(),
            graph: DiGraph::new(),
            name_registry: HashMap::new(),
            node_set: HashMap::new(),
        }
    }

    /// Parse the wiki directory and rebuild the graph from scratch.
    pub fn rebuild(&mut self) {
        self.graph.clear();
        self.name_registry.clear();
        self.node_set.clear();

        let parsed = parse_index(&self.wiki_dir);

        // First pass: add nodes, extract cross-refs.
        let mut entries_with_refs: Vec<(
            String,
            WikiCategory,
            PathBuf,
            String,
            Vec<String>,
            NodeIndex,
        )> = Vec::new();

        for (category, name, rel_path) in &parsed {
            let file_path = self.wiki_dir.join(rel_path);
            let (title, cross_refs) = extract_cross_refs(&file_path);

            let idx = self.graph.add_node(WikiNode {
                name: name.clone(),
                category: category.clone(),
                title: if title.is_empty() {
                    name.clone()
                } else {
                    title.clone()
                },
                rel_path: rel_path.clone(),
                first_seen: None,
                last_seen: None,
                observation_count: 0,
            });

            self.node_set.insert(name.clone(), idx);
            entries_with_refs.push((
                name.clone(),
                category.clone(),
                rel_path.clone(),
                title,
                cross_refs,
                idx,
            ));
        }

        // Build name registry.
        let registry_input: Vec<(String, String, PathBuf, NodeIndex)> = entries_with_refs
            .iter()
            .map(|(name, _, path, title, _, idx)| (name.clone(), title.clone(), path.clone(), *idx))
            .collect();
        self.name_registry = populate_registry(&registry_input);

        // Second pass: add edges from cross-references.
        for (name, _, _, _, cross_refs, src_idx) in &entries_with_refs {
            for ref_text in cross_refs {
                if let Some(target_idx) = match_reference(ref_text, &self.name_registry) {
                    // No self-edges.
                    if target_idx != *src_idx {
                        // Avoid duplicate edges.
                        if !self.graph.edges(*src_idx).any(|e| e.target() == target_idx) {
                            self.graph.add_edge(
                                *src_idx,
                                target_idx,
                                WikiEdge {
                                    ref_text: ref_text.clone(),
                                    first_seen: None,
                                    last_seen: None,
                                    evidence_sources: Vec::new(),
                                },
                            );
                        }
                    }
                }
            }
            let _ = name; // silence unused binding
        }
    }

    /// Number of nodes in the graph.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Returns the number of edges in the graph.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // petgraph DiGraph::edge_count is not const
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Get the underlying graph.
    #[must_use]
    pub const fn graph(&self) -> &DiGraph<WikiNode, WikiEdge> {
        &self.graph
    }

    /// Get the name registry (lowered key → `NodeIndex`).
    #[must_use]
    pub const fn registry(&self) -> &HashMap<String, NodeIndex> {
        &self.name_registry
    }

    /// Look up a node by canonical name.
    #[must_use]
    pub fn node_by_name(&self, name: &str) -> Option<NodeIndex> {
        self.node_set.get(name).copied()
    }

    /// Wiki directory path.
    #[must_use]
    pub const fn wiki_dir(&self) -> &PathBuf {
        &self.wiki_dir
    }

    /// Convert a `WikiEntry` (domain type) from a node.
    #[must_use]
    pub fn to_wiki_entry(&self, idx: NodeIndex) -> Option<WikiEntry> {
        self.graph.node_weight(idx).map(|node| WikiEntry {
            path: node.rel_path.clone(),
            title: node.title.clone(),
            category: node.category.clone(),
            cross_refs: self
                .graph
                .edges(idx)
                .map(|e| e.weight().ref_text.clone())
                .collect(),
        })
    }
}

// ── WikiWatcher (async, non-blocking) ───────────────────────────────────────

/// Polls the wiki directory for `.md` file changes and triggers a rebuild.
///
/// `Send + Sync` and does not block the async runtime.
#[cfg(feature = "runtime")]
pub struct WikiWatcher {
    wiki_dir: PathBuf,
    model: Arc<RwLock<WikiGraphModel>>,
    cancel: tokio_util::sync::CancellationToken,
}

#[cfg(feature = "runtime")]
impl WikiWatcher {
    pub fn new(wiki_dir: impl Into<PathBuf>, model: Arc<RwLock<WikiGraphModel>>) -> Self {
        Self {
            wiki_dir: wiki_dir.into(),
            model,
            cancel: tokio_util::sync::CancellationToken::new(),
        }
    }

    /// Start the background poll loop. Returns a [`tokio::task::JoinHandle`].
    #[must_use]
    pub fn start(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            self.poll_loop().await;
        })
    }

    /// Signal the watcher to stop.
    pub fn stop(&self) {
        self.cancel.cancel();
    }

    async fn poll_loop(&self) {
        let mut last_snapshot = self.snapshot();
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
        interval.tick().await; // consume first immediate tick

        loop {
            tokio::select! {
                () = self.cancel.cancelled() => break,
                _ = interval.tick() => {}
            }

            let new_snapshot = self.snapshot();
            if new_snapshot != last_snapshot {
                last_snapshot = new_snapshot;
                let mut model = self.model.write().await;
                model.rebuild();
            }
        }
    }

    fn snapshot(&self) -> HashMap<PathBuf, u64> {
        let mut result = HashMap::new();
        if let Ok(walker) = walkdir(&self.wiki_dir) {
            for (path, mtime) in walker {
                result.insert(path, mtime);
            }
        }
        result
    }
}

/// Walk a directory and collect `(path, mtime_secs)` for `.md` files.
#[cfg(feature = "runtime")]
fn walkdir(dir: &Path) -> std::io::Result<Vec<(PathBuf, u64)>> {
    let mut results = Vec::new();
    walkdir_inner(dir, &mut results)?;
    Ok(results)
}

#[cfg(feature = "runtime")]
fn walkdir_inner(dir: &Path, results: &mut Vec<(PathBuf, u64)>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walkdir_inner(&path, results)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("md")
            && let Ok(meta) = std::fs::metadata(&path)
        {
            use std::time::UNIX_EPOCH;
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            results.push((path, mtime));
        }
    }
    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[cfg(feature = "runtime")]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_wiki(dir: &Path) {
        fs::create_dir_all(dir.join("campaign-finance")).unwrap();
        fs::create_dir_all(dir.join("corporate")).unwrap();
        fs::create_dir_all(dir.join("lobbying")).unwrap();

        fs::write(
            dir.join("index.md"),
            r"# Data Sources Wiki

### Campaign Finance

| Source | Jurisdiction | Link |
|--------|-------------|------|
| FEC Federal | US federal | [fec-federal.md](campaign-finance/fec-federal.md) |

### Corporate Registries

| Source | Jurisdiction | Link |
|--------|-------------|------|
| SEC EDGAR | US public | [sec-edgar.md](corporate/sec-edgar.md) |
| Acme Corp Registry | Massachusetts | [acme-corp.md](corporate/acme-corp.md) |

### Lobbying

| Source | Jurisdiction | Link |
|--------|-------------|------|
| Senate Lobbying Disclosures (LD-1/LD-2) | US federal | [senate-ld.md](lobbying/senate-ld.md) |
",
        )
        .unwrap();

        fs::write(
            dir.join("campaign-finance/fec-federal.md"),
            r"# FEC Federal Campaign Finance

## Summary
Federal campaign finance data.

## Cross-Reference Potential

- **SEC EDGAR** filings for corporate donors
- **Senate Lobbying Disclosures** for lobbyist-donor connections
- **Acme Corp** as a known entity
",
        )
        .unwrap();

        fs::write(
            dir.join("corporate/sec-edgar.md"),
            r"# SEC EDGAR

## Summary
SEC corporate filings.

## Cross-Reference Potential

- **FEC Federal** campaign contributions from corporate officers
",
        )
        .unwrap();

        fs::write(
            dir.join("corporate/acme-corp.md"),
            r"# Acme Corporation

## Summary
A test corporation.

## Cross-Reference Potential

- **FEC Federal Campaign Finance** donations by Acme employees
",
        )
        .unwrap();

        fs::write(
            dir.join("lobbying/senate-ld.md"),
            r"# Senate Lobbying Disclosures

## Summary
Federal lobbying disclosures.

## Cross-Reference Potential

- **FEC Federal** for related campaign contributions
- **SEC EDGAR** for corporate lobbying entity matches
",
        )
        .unwrap();
    }

    #[test]
    fn parse_index_produces_correct_triples() {
        let tmp = TempDir::new().unwrap();
        let wiki = tmp.path();
        make_wiki(wiki);

        let entries = parse_index(wiki);
        assert_eq!(entries.len(), 4);

        assert_eq!(entries[0].0, WikiCategory::CampaignFinance);
        assert_eq!(entries[0].1, "FEC Federal");
        assert_eq!(
            entries[0].2,
            PathBuf::from("campaign-finance/fec-federal.md")
        );

        assert_eq!(entries[1].0, WikiCategory::Corporate);
        assert_eq!(entries[1].1, "SEC EDGAR");

        assert_eq!(entries[2].0, WikiCategory::Corporate);
        assert_eq!(entries[2].1, "Acme Corp Registry");

        assert_eq!(entries[3].0, WikiCategory::Lobbying);
        assert_eq!(entries[3].1, "Senate Lobbying Disclosures (LD-1/LD-2)");
    }

    #[test]
    fn extract_bold_cross_refs() {
        let tmp = TempDir::new().unwrap();
        let wiki = tmp.path();
        make_wiki(wiki);

        let (title, refs) = extract_cross_refs(&wiki.join("campaign-finance/fec-federal.md"));
        assert_eq!(title, "FEC Federal Campaign Finance");
        assert_eq!(refs.len(), 3);
        assert!(refs.contains(&"SEC EDGAR".to_owned()));
        assert!(refs.contains(&"Senate Lobbying Disclosures".to_owned()));
        assert!(refs.contains(&"Acme Corp".to_owned()));
    }

    #[test]
    fn rebuild_creates_graph_with_nodes_and_edges() {
        let tmp = TempDir::new().unwrap();
        let wiki = tmp.path();
        make_wiki(wiki);

        let mut model = WikiGraphModel::new(wiki);
        model.rebuild();

        assert_eq!(model.node_count(), 4);
        assert!(model.edge_count() > 0, "Should have cross-ref edges");
    }

    #[test]
    fn fuzzy_match_finds_acme_corporation() {
        let tmp = TempDir::new().unwrap();
        let wiki = tmp.path();
        make_wiki(wiki);

        let mut model = WikiGraphModel::new(wiki);
        model.rebuild();

        // "Acme Corp" should match "Acme Corp Registry" node via substring.
        let result = match_reference("Acme Corp", &model.name_registry);
        assert!(result.is_some(), "Should match Acme Corp Registry");
    }

    #[test]
    fn acronym_resolves_parenthetical() {
        let tmp = TempDir::new().unwrap();
        let wiki = tmp.path();
        make_wiki(wiki);

        let mut model = WikiGraphModel::new(wiki);
        model.rebuild();

        // "LD-1/LD-2" is the parenthetical from "Senate Lobbying Disclosures (LD-1/LD-2)".
        let result = match_reference("LD-1/LD-2", &model.name_registry);
        assert!(result.is_some(), "Should match via parenthetical key");
    }

    #[test]
    fn rebuild_after_file_write_updates_graph() {
        let tmp = TempDir::new().unwrap();
        let wiki = tmp.path();
        make_wiki(wiki);

        let mut model = WikiGraphModel::new(wiki);
        model.rebuild();
        let old_count = model.node_count();

        // Add a new entry to the index.
        let index = fs::read_to_string(wiki.join("index.md")).unwrap();
        let new_index = index.replace(
            "### Lobbying",
            "### Financial\n\n| Source | Jurisdiction | Link |\n|--------|-------------|------|\n| FDIC BankFind | US banks | [fdic.md](financial/fdic.md) |\n\n### Lobbying",
        );
        fs::create_dir_all(wiki.join("financial")).unwrap();
        fs::write(wiki.join("index.md"), new_index).unwrap();
        fs::write(
            wiki.join("financial/fdic.md"),
            "# FDIC BankFind\n\n## Summary\nBank data.\n\n## Cross-Reference Potential\n\n- **SEC EDGAR** for bank holding companies\n",
        )
        .unwrap();

        model.rebuild();
        assert!(
            model.node_count() > old_count,
            "Node count should increase after adding entry"
        );
    }

    #[test]
    fn negative_fuzzy_match() {
        let tmp = TempDir::new().unwrap();
        let wiki = tmp.path();
        make_wiki(wiki);

        let mut model = WikiGraphModel::new(wiki);
        model.rebuild();

        // Completely unrelated string should not match.
        let result = match_reference("Zebra Quantum Computing LLC", &model.name_registry);
        assert!(result.is_none(), "Should not match unrelated string");
    }

    #[test]
    fn jaro_winkler_identical_strings() {
        assert!((jaro_winkler("hello", "hello") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn jaro_winkler_similar_strings() {
        let score = jaro_winkler("acme corporation", "acme corp");
        assert!(
            score >= 0.88,
            "Similar strings should score ≥ 0.88, got {score}"
        );
    }

    #[test]
    fn jaro_winkler_dissimilar_strings() {
        let score = jaro_winkler("zebra quantum", "acme corp");
        assert!(
            score < 0.88,
            "Dissimilar strings should score < 0.88, got {score}"
        );
    }

    #[test]
    fn category_color_returns_correct_color() {
        assert_eq!(category_color(&WikiCategory::CampaignFinance), "Cyan");
        assert_eq!(category_color(&WikiCategory::Financial), "Red");
        assert_eq!(category_color(&WikiCategory::Other), "Gray");
    }

    #[test]
    fn no_self_edges() {
        let tmp = TempDir::new().unwrap();
        let wiki = tmp.path();
        make_wiki(wiki);

        let mut model = WikiGraphModel::new(wiki);
        model.rebuild();

        for edge in model.graph().edge_indices() {
            let (src, dst) = model.graph().edge_endpoints(edge).unwrap();
            assert_ne!(src, dst, "Graph should have no self-edges");
        }
    }

    #[test]
    fn to_wiki_entry_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let wiki = tmp.path();
        make_wiki(wiki);

        let mut model = WikiGraphModel::new(wiki);
        model.rebuild();

        let idx = model.node_by_name("FEC Federal").expect("node exists");
        let entry = model.to_wiki_entry(idx).expect("entry exists");
        assert_eq!(entry.title, "FEC Federal Campaign Finance");
        assert_eq!(entry.category, WikiCategory::CampaignFinance);
    }
}
