//! Gaiji (外字) lookup and search.
//!
//! A bidirectional index over the gaiji corpus: description ⇔ char ⇔ JIS
//! 面区点 (men-ku-ten) ⇔ Unicode, plus fuzzy description search. The forward
//! direction is differentially checked against
//! `aozora::encoding::gaiji::lookup` so this index never disagrees with the
//! parser's authoritative resolution.
//!
//! Implemented in a later milestone; this module anchors the layout.
