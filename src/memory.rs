use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryProvider {
    AutoMem,
    Icm,
    Skip,
}

impl fmt::Display for MemoryProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AutoMem => write!(
                f,
                "AutoMem — graph memory, needs Node.js + external services"
            ),
            Self::Icm => write!(f, "ICM — embedded SQLite, zero dependencies"),
            Self::Skip => write!(f, "Skip — no memory provider"),
        }
    }
}

impl MemoryProvider {
    pub const CHOICES: [Self; 3] = [Self::Icm, Self::AutoMem, Self::Skip];

    pub fn name(&self) -> &'static str {
        match self {
            Self::AutoMem => "AutoMem",
            Self::Icm => "ICM",
            Self::Skip => "none",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_includes_description() {
        let s = format!("{}", MemoryProvider::Icm);
        assert!(s.contains("ICM"));
        assert!(s.contains("zero dependencies"));
    }

    #[test]
    fn choices_default_is_icm() {
        assert_eq!(MemoryProvider::CHOICES[0], MemoryProvider::Icm);
    }

    #[test]
    fn name_returns_short_label() {
        assert_eq!(MemoryProvider::AutoMem.name(), "AutoMem");
        assert_eq!(MemoryProvider::Icm.name(), "ICM");
        assert_eq!(MemoryProvider::Skip.name(), "none");
    }
}
