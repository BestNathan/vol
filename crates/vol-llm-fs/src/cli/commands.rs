//! Parsed CLI command variants for the fs tool.
//!
//! Each variant maps to a specific subcommand of the `fs` CLI. The parser
//! (see [`super::parser`]) yields one of these values for every successfully
//! parsed invocation.

/// Parsed CLI command — one variant per subcommand.
#[derive(Debug, Clone)]
pub(crate) enum ParsedCommand {
    /// `fs read --file_path <PATH> [--offset N] [--limit N]`
    Read {
        file_path: String,
        offset: usize,
        limit: usize,
        json: bool,
    },
    /// `fs write --file_path <PATH> --content <TEXT>`
    Write {
        file_path: String,
        content: String,
        json: bool,
    },
    /// `fs edit --file_path <PATH> --old_string <S> --new_string <S> [--replace_all]`
    Edit {
        file_path: String,
        old_string: String,
        new_string: String,
        replace_all: bool,
        json: bool,
    },
    /// `fs grep --pattern <REGEX> [--path DIR] [--glob PAT] [--output_mode M] [--case_sensitive]`
    Grep {
        pattern: String,
        path: Option<String>,
        glob: Option<String>,
        output_mode: String,
        case_sensitive: bool,
        json: bool,
    },
    /// `fs glob --pattern <PAT> [--path DIR] [--exclude A,B] ...`
    Glob {
        pattern: String,
        path: String,
        exclude: Option<Vec<String>>,
        kind: String,
        max_results: usize,
        include_hidden: bool,
        follow_symlinks: bool,
        sort: String,
        with_metadata: bool,
        json: bool,
    },
    /// `fs scheme [<subcommand>]`
    Scheme { subcommand: Option<String> },
}
