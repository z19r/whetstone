use std::fmt;

/// The persistent-memory provider whetstone wires in for a project.
///
/// v3 collapsed the v2 trio `{Icm, AutoMem, Skip}` down to `{Icm, Skip}`.
/// AutoMem was removed in Phase 1 task 1.3 — see `docs/v3/WHETSTONE_V3_PLAN.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryProvider {
    Icm,
    Skip,
}

impl fmt::Display for MemoryProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Icm => write!(f, "ICM — embedded SQLite, zero dependencies"),
            Self::Skip => write!(f, "Skip — no memory provider"),
        }
    }
}

impl MemoryProvider {
    pub const CHOICES: [Self; 2] = [Self::Icm, Self::Skip];

    pub fn name(&self) -> &'static str {
        match self {
            Self::Icm => "ICM",
            Self::Skip => "none",
        }
    }
}
