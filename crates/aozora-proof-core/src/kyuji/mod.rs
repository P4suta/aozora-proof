//! Old-/new-form kanji (旧字体↔新字体) detection and replacement suggestions.
//!
//! For each character that has an old/new-form counterpart, emits an
//! `aozora::kyuji::*` finding carrying a [`crate::Suggestion`]. Suggestion-only
//! by design — it flags candidates for the editor to confirm against the
//! 底本 (source edition); it never auto-replaces.
//!
//! Implemented in a later milestone; this module anchors the layout.
