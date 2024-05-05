use clap::ValueEnum;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum GenshinArtifactExportFormat {
    Mona,
    MingyuLab,
    Good,
    CSV,
}

impl Default for GenshinArtifactExportFormat {
    fn default() -> Self {
        Self::Mona
    }
}
