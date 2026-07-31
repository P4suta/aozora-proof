use std::path::PathBuf;

use aozora_proof_core::Orthography;
use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

#[derive(Debug, Parser)]
#[command(
    name = "aozora-proof",
    version = crate::LONG_VERSION,
    about = "Submission-quality proofreading for Aozora Bunko text",
    after_help = "Examples:
  aozora-proof check --orthography modern manuscript.txt
  aozora-proof fix --orthography mixed --dry-run .
  aozora-proof review --orthography traditional manuscript.txt
  aozora-proof gaiji lookup U+4FF1
  aozora-proof explain aozora::proof::encoding::line_ending"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,

    #[arg(long, value_enum, global = true)]
    pub(crate) color: Option<ColorChoice>,

    #[arg(long, value_enum, global = true)]
    pub(crate) lang: Option<LanguageArg>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Evaluate every submission requirement without modifying input.
    Check(CheckArgs),
    /// Apply only fixes classified as safe.
    Fix(FixArgs),
    /// Stage judgement-dependent changes in a full-screen terminal.
    Review(ReviewArgs),
    /// Explain one stable rule code.
    Explain {
        /// Rule code.
        code: String,
    },
    /// Look up and search external characters.
    Gaiji {
        #[command(subcommand)]
        command: GaijiCommand,
    },
    /// List automatic, review, and manual requirements.
    Rules {
        #[arg(long, value_enum)]
        format: Option<Format>,
    },
    /// Interactively create configuration.
    Init(InitArgs),
    /// Inspect configuration.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Generate shell completions from this command definition.
    Completions {
        /// Target shell.
        shell: Shell,
    },
    /// Generate the manual page from this command definition.
    Man,
}

#[derive(Debug, Args)]
pub(crate) struct CheckArgs {
    /// Files, directories, or `-`; no argument reads standard input.
    pub(crate) paths: Vec<PathBuf>,

    /// Re-run when files, directories, or configuration change.
    #[arg(long)]
    pub(crate) watch: bool,

    /// Output format.
    #[arg(long, value_enum)]
    pub(crate) format: Option<Format>,

    /// Finding threshold that produces exit code 1.
    #[arg(long, value_enum)]
    pub(crate) fail_on: Option<SeverityArg>,

    #[command(flatten)]
    pub(crate) document: DocumentArgs,
}

#[derive(Debug, Args)]
pub(crate) struct FixArgs {
    /// Files, directories, or `-`; no argument reads standard input.
    pub(crate) paths: Vec<PathBuf>,

    /// Print a unified diff without writing.
    #[arg(short = 'n', long)]
    pub(crate) dry_run: bool,

    #[command(flatten)]
    pub(crate) document: DocumentArgs,
}

#[derive(Debug, Args)]
pub(crate) struct ReviewArgs {
    /// Files or directories. Standard input is not supported.
    pub(crate) paths: Vec<PathBuf>,

    #[command(flatten)]
    pub(crate) document: DocumentArgs,
}

#[derive(Debug, Args)]
pub(crate) struct DocumentArgs {
    /// Directional character-form policy.
    #[arg(long, value_enum)]
    pub(crate) orthography: Option<OrthographyArg>,

    /// Never prompt for a missing policy.
    #[arg(long)]
    pub(crate) no_input: bool,

    /// Read this configuration file instead of discovering a project file.
    #[arg(long, value_name = "FILE")]
    pub(crate) config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct InitArgs {
    /// Create the platform user configuration instead of a project file.
    #[arg(long)]
    pub(crate) user: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum GaijiCommand {
    /// Look up a character, U+ code point, or men-ku-ten address.
    Lookup {
        /// Character, `U+XXXX`, or `M-K-T`.
        query: String,
    },
    /// Search official annotation descriptions.
    Search {
        /// Description substring.
        text: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ConfigCommand {
    /// Show effective values and their source.
    Show {
        /// Optional input path used to discover the nearest project file.
        path: Option<PathBuf>,
    },
    /// Print the configuration JSON Schema.
    Schema,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub(crate) enum Format {
    Auto,
    Human,
    Json,
    Short,
    Sarif,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub(crate) enum SeverityArg {
    Error,
    Warning,
    Note,
}

impl SeverityArg {
    pub(crate) const fn rank(self) -> u8 {
        match self {
            Self::Error => 3,
            Self::Warning => 2,
            Self::Note => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub(crate) enum OrthographyArg {
    Modern,
    Traditional,
    Mixed,
}

impl From<OrthographyArg> for Orthography {
    fn from(value: OrthographyArg) -> Self {
        match value {
            OrthographyArg::Modern => Self::Modern,
            OrthographyArg::Traditional => Self::Traditional,
            OrthographyArg::Mixed => Self::Mixed,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub(crate) enum LanguageArg {
    En,
    Ja,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub(crate) enum ColorChoice {
    Auto,
    Always,
    Never,
}
