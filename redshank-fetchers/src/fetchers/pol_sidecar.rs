//! Shared helpers for emitting `PoL` (Proof-of-Liveness) observation sidecars.
//!
//! Each fetcher that wants to emit an append-only NDJSON sidecar can call these
//! helpers to compute a CRC32 content hash, detect delta classification, read the
//! most recent observation for an entity, and append a new one.
//!
//! The read path uses a tail-scan: it reads at most the last [`MAX_TAIL_BYTES`] of
//! the sidecar file and scans backwards, so the common case (latest entry near EOF)
//! is O(1) rather than O(n). A full forward scan is used as a fallback only when
//! the file exceeds [`MAX_TAIL_BYTES`] and the entity was not found in the tail.

use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use redshank_core::domain::observation::{EntityObservation, ObservationDelta};

use crate::domain::FetchError;

/// Maximum bytes to read from the end of a sidecar file before falling back to
/// a full forward scan. Covers ~200–300 JSON observation records.
const MAX_TAIL_BYTES: u64 = 65_536;

/// Compute a CRC32 hex digest of the JSON-serialised form of `payload`.
///
/// # Errors
///
/// Returns [`FetchError::Parse`] if `payload` cannot be serialised.
pub fn snapshot_payload_hash<T: serde::Serialize>(
    payload: &T,
) -> Result<String, FetchError> {
    let bytes = serde_json::to_vec(payload)
        .map_err(|e| FetchError::Parse(format!("serialize hash payload: {e}")))?;
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(&bytes);
    Ok(format!("{:08x}", hasher.finalize()))
}

/// Classify the delta between a previous observation and a new content hash.
#[must_use]
pub fn classify_delta(
    previous: Option<&EntityObservation>,
    payload_hash: &str,
) -> ObservationDelta {
    match previous {
        None => ObservationDelta::New,
        Some(prev) if prev.payload_hash == payload_hash => ObservationDelta::Unchanged,
        Some(prev) => ObservationDelta::Changed {
            previous_hash: prev.payload_hash.clone(),
        },
    }
}

/// Return the most recent observation for `(entity_id, source_id)` from `path`.
///
/// Uses a tail-scan (reads from EOF backwards) so the common case is O(1).
/// Falls back to a full forward scan when the file exceeds [`MAX_TAIL_BYTES`] and
/// the entity is not found in the tail region.
///
/// # Errors
///
/// Returns [`FetchError`] on I/O or JSON parse failures.
pub fn read_latest_observation(
    path: &Path,
    entity_id: &str,
    source_id: &str,
) -> Result<Option<EntityObservation>, FetchError> {
    use std::io::BufRead;

    if !path.exists() {
        return Ok(None);
    }

    let mut file = std::fs::File::open(path)?;
    let file_len = file.seek(SeekFrom::End(0))?;
    if file_len == 0 {
        return Ok(None);
    }

    // Tail scan: read at most MAX_TAIL_BYTES from the end.
    let start_offset = file_len.saturating_sub(MAX_TAIL_BYTES);
    file.seek(SeekFrom::Start(start_offset))?;

    let capacity = usize::try_from(file_len - start_offset).unwrap_or_default();
    let mut buf = Vec::with_capacity(capacity);
    file.read_to_end(&mut buf)?;

    let text = std::str::from_utf8(&buf)
        .map_err(|e| FetchError::Parse(format!("observation sidecar utf8: {e}")))?;

    // When start_offset > 0 the first bytes may be mid-record; skip to next newline.
    let scan_text: &str = if start_offset > 0 {
        text.find('\n').map_or("", |i| &text[i + 1..])
    } else {
        text
    };

    // Scan lines in reverse: newest-appended entries are last, so the first match
    // when scanning backwards is the most recent for this entity/source.
    for line in scan_text.lines().rev() {
        if !line.trim().is_empty()
            && let Ok(obs) = serde_json::from_str::<EntityObservation>(line)
            && obs.entity_id == entity_id
            && obs.source_id == source_id
        {
            return Ok(Some(obs));
        }
    }

    // Fallback: when the file exceeds MAX_TAIL_BYTES and the tail search found no
    // match, do a full forward scan to avoid false negatives.
    if start_offset > 0 {
        file.seek(SeekFrom::Start(0))?;
        let reader = std::io::BufReader::new(file);
        let mut latest: Option<EntityObservation> = None;
        for line_result in reader.lines() {
            let line = line_result?;
            if !line.trim().is_empty()
                && let Ok(obs) = serde_json::from_str::<EntityObservation>(&line)
                && obs.entity_id == entity_id
                && obs.source_id == source_id
                && latest
                    .as_ref()
                    .is_none_or(|cur| obs.observed_at > cur.observed_at)
            {
                latest = Some(obs);
            }
        }
        return Ok(latest);
    }

    Ok(None)
}

/// Append `observation` as a JSON line to the sidecar NDJSON file at `path`.
///
/// # Errors
///
/// Returns [`FetchError`] on I/O or serialisation failures.
pub fn append_observation(
    path: &Path,
    observation: &EntityObservation,
) -> Result<(), FetchError> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let line = serde_json::to_string(observation)
        .map_err(|e| FetchError::Parse(format!("serialize observation: {e}")))?;
    writeln!(file, "{line}")?;
    file.flush()?;
    Ok(())
}
