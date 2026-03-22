//! # mana-review
//!
//! Code review engine for the mana work coordination system.
//!
//! `mana-review` provides the review workflow for agent-produced code:
//! risk scoring, diff analysis, review state management, structured
//! feedback, and HTML viewer generation.
//!
//! ## Design principles
//!
//! 1. **Context-first** — every review starts with unit context (description,
//!    verify result, attempts, dependencies), not just a diff.
//! 2. **Heuristic risk scoring** — cheap, fast, deterministic signals first.
//!    LLM-based analysis is optional and comes later.
//! 3. **Structured feedback** — review annotations are stored in `.mana/`
//!    and injected into the next agent's context on retry.
//! 4. **Attribution tracking** — record what the reviewer does with each
//!    finding to learn what's useful over time.
//! 5. **HTML viewer** — generates a self-contained HTML page that works
//!    in any browser today and in Sourcery/Photon tomorrow.
//!
//! ## Crate layout
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`types`] | Core review types — decisions, annotations, risk levels |
//! | [`risk`] | Heuristic risk scoring for review triage |
//! | [`diff`] | Git diff computation and file change analysis |
//! | [`feedback`] | Structured review feedback for agent retry context |
//! | [`queue`] | Review queue — list and rank units awaiting review |
//! | [`render`] | HTML review page generation |
//! | [`state`] | Review state persistence in `.mana/` |

pub mod diff;
pub mod feedback;
pub mod queue;
pub mod render;
pub mod risk;
pub mod state;
pub mod types;
