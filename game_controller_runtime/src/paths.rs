//! This module locates the files that belong to an installation of the GameController: the
//! configuration directory (teams and competitions) and the optional per-user options file.
//!
//! The configuration directory is the first of the following candidates that contains a
//! `teams.yaml`:
//!
//! 1. an explicitly requested directory (`--config-dir` or `GAMECONTROLLER_CONFIG`); if this one is
//!    given, there is no fallback,
//! 2. the `config` directory in the per-user directory (see [user_config_directory]),
//! 3. the directory that is installed or bundled next to the executable,
//! 4. the directory of a source checkout or of the legacy archive layout, `<exe>/../../config`.

use std::{
    env::var_os,
    fmt::{self, Display, Formatter},
    fs::{copy, create_dir_all, read_dir, read_to_string, write},
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};

/// The name of the file that marks a directory as a configuration directory.
const MARKER_FILE: &str = "teams.yaml";

/// The name of the file in the user's configuration directory that stores the version which installed the config.
const VERSION_STAMP_FILE: &str = ".version";

/// The name of the file in the user directory that contains persistent command line arguments.
const ARGS_FILE: &str = "args";

/// The environment variable that is equivalent to the `--config-dir` option.
const CONFIG_ENV: &str = "GAMECONTROLLER_CONFIG";

/// The template that is written to a new options file.
const ARGS_TEMPLATE: &str = "\
# Command line options in this file are used on every start of the GameController. Options on the
# real command line take precedence. Put whitespace-separated options here (quotes and comments
# work as in a shell), for example:
#
# --interface en0
# --sync
";

/// This enumerates the places where a configuration directory can come from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigOrigin {
    /// The directory was requested with `--config-dir` or the environment.
    Explicit,
    /// The per-user directory.
    User,
    /// The directory installed with or bundled in the application.
    Installed,
    /// The directory in a source checkout or in the legacy archive layout.
    Development,
}

impl Display for ConfigOrigin {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ConfigOrigin::Explicit => "explicitly requested",
            ConfigOrigin::User => "user",
            ConfigOrigin::Installed => "installed",
            ConfigOrigin::Development => "development",
        })
    }
}

/// This struct is the result of the search for a configuration directory.
#[derive(Clone, Debug)]
pub struct ResolvedConfig {
    /// The directory that is used.
    pub directory: PathBuf,
    /// Why that directory was chosen.
    pub origin: ConfigOrigin,
}

impl Display for ResolvedConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.directory.display(), self.origin)
    }
}

/// This function returns the value of an environment variable as a path if it is not empty.
fn non_empty_path(name: &str) -> Option<PathBuf> {
    var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// This function returns the per-user directory of the GameController, which contains the
/// options file and the `config` directory (and the `logs` directory except on Linux). It is
/// `$XDG_CONFIG_HOME/GameController` (or `~/.config/GameController`) on Linux,
/// `~/Library/Application Support/GameController` on macOS and `%LOCALAPPDATA%\GameController` on
/// Windows. It is `None` if the base directory is unknown.
pub fn user_directory() -> Option<PathBuf> {
    let base = if cfg!(target_os = "windows") {
        non_empty_path("LOCALAPPDATA")?
    } else if cfg!(target_os = "macos") {
        non_empty_path("HOME")?
            .join("Library")
            .join("Application Support")
    } else {
        // The XDG specification requires ignoring relative paths.
        non_empty_path("XDG_CONFIG_HOME")
            .filter(|p| p.is_absolute())
            .or_else(|| non_empty_path("HOME").map(|home| home.join(".config")))?
    };
    Some(base.join("GameController"))
}

/// This function returns the directory with the user's own configuration.
pub fn user_config_directory() -> Option<PathBuf> {
    user_directory().map(|directory| directory.join("config"))
}

/// This function returns the directory for log files. It is in the per-user directory, except on
/// Linux where it is `$XDG_DATA_HOME/GameController/logs` (or `~/.local/share/GameController/logs`).
pub fn user_log_directory() -> Option<PathBuf> {
    if cfg!(any(target_os = "windows", target_os = "macos")) {
        user_directory().map(|directory| directory.join("logs"))
    } else {
        // The XDG specification requires ignoring relative paths.
        let base = non_empty_path("XDG_DATA_HOME")
            .filter(|p| p.is_absolute())
            .or_else(|| non_empty_path("HOME").map(|home| home.join(".local").join("share")))?;
        Some(base.join("GameController").join("logs"))
    }
}

/// This function returns the explicitly requested configuration directory from the environment.
fn explicit_from_environment() -> Option<PathBuf> {
    var_os(CONFIG_ENV)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// This function returns the directory with the configuration that is shipped with the
/// application for an executable at the given path.
fn installed_directory(exe: &Path) -> Option<PathBuf> {
    let exe_directory = exe.parent()?;
    Some(if cfg!(target_os = "windows") {
        exe_directory.join("config")
    } else if cfg!(target_os = "macos") {
        // <bundle>/Contents/MacOS/GameController
        exe_directory.join("..").join("Resources").join("config")
    } else {
        // <prefix>/bin/GameController
        exe_directory
            .join("..")
            .join("share")
            .join("GameController")
            .join("config")
    })
}

/// This function returns all candidates for the configuration directory in order of priority.
pub fn config_candidates(
    explicit: Option<&Path>,
    user: Option<&Path>,
    exe: &Path,
) -> Vec<(ConfigOrigin, PathBuf)> {
    if let Some(explicit) = explicit {
        return vec![(ConfigOrigin::Explicit, explicit.to_path_buf())];
    }
    let mut candidates = vec![];
    if let Some(user) = user {
        candidates.push((ConfigOrigin::User, user.to_path_buf()));
    }
    if let Some(installed) = installed_directory(exe) {
        candidates.push((ConfigOrigin::Installed, installed));
    }
    if let Some(exe_directory) = exe.parent() {
        candidates.push((
            ConfigOrigin::Development,
            exe_directory.join("..").join("..").join("config"),
        ));
    }
    candidates
}

/// This function finds the configuration directory. `explicit` is the value of `--config-dir`;
/// if it is `None`, the environment is consulted. The error lists all candidates that were tried.
pub fn resolve_config_directory(
    explicit: Option<&Path>,
    user: Option<&Path>,
    exe: &Path,
) -> Result<ResolvedConfig> {
    let from_environment = explicit_from_environment();
    let explicit = explicit.or(from_environment.as_deref());
    let candidates = config_candidates(explicit, user, exe);
    for (origin, directory) in &candidates {
        if directory.join(MARKER_FILE).is_file() {
            return Ok(ResolvedConfig {
                directory: directory.clone(),
                origin: *origin,
            });
        }
    }
    let tried = candidates
        .iter()
        .map(|(origin, directory)| {
            format!(
                "  {} ({}): no {MARKER_FILE} found",
                directory.display(),
                origin
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    bail!("could not find a configuration directory, tried:\n{tried}")
}

/// This function returns a warning if the user's configuration directory was set up by another
/// version than the running one and is therefore possibly outdated.
pub fn stale_config_warning(config: &ResolvedConfig) -> Option<String> {
    if config.origin != ConfigOrigin::User {
        return None;
    }
    let current = env!("CARGO_PKG_VERSION");
    let stamp = read_to_string(config.directory.join(VERSION_STAMP_FILE)).ok();
    match stamp.as_deref().map(str::trim) {
        Some(stamp) if stamp == current => None,
        Some(stamp) => Some(format!(
            "the configuration in {} was installed by version {stamp}, but this is {current}; \
            it may be missing changes (see --install-config)",
            config.directory.display()
        )),
        None => Some(format!(
            "the configuration in {} has no version information; it may be outdated \
            (see --install-config)",
            config.directory.display()
        )),
    }
}

/// This function copies a directory recursively.
fn copy_recursively(from: &Path, to: &Path) -> Result<()> {
    create_dir_all(to).with_context(|| format!("could not create {}", to.display()))?;
    for entry in read_dir(from).with_context(|| format!("could not read {}", from.display()))? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_recursively(&entry.path(), &target)?;
        } else {
            copy(entry.path(), &target)
                .with_context(|| format!("could not copy to {}", target.display()))?;
        }
    }
    Ok(())
}

/// This function copies the installed configuration to the `config` directory in the user
/// directory so that it can be edited, writes a version stamp and creates an options file template
/// if there is none. It refuses to overwrite an existing user configuration.
pub fn install_user_config(user: &Path, exe: &Path) -> Result<()> {
    let config = user.join("config");
    if config.join(MARKER_FILE).exists() {
        bail!(
            "{} already contains a configuration, remove it first to start over",
            config.display()
        );
    }
    let candidates = config_candidates(None, None, exe);
    let Some((origin, source)) = candidates
        .iter()
        .find(|(_, directory)| directory.join(MARKER_FILE).is_file())
    else {
        bail!("could not find a bundled configuration to copy");
    };
    copy_recursively(source, &config)?;
    write(config.join(VERSION_STAMP_FILE), env!("CARGO_PKG_VERSION"))
        .context("could not write the version stamp")?;
    let args_file = user.join(ARGS_FILE);
    if !args_file.exists() {
        write(&args_file, ARGS_TEMPLATE).context("could not write the options file")?;
    }
    println!(
        "Copied the {origin} configuration {} to {}",
        source.display(),
        config.display()
    );
    println!(
        "Options for every start can be put in {}",
        args_file.display()
    );
    Ok(())
}

/// This function splits the contents of an options file into arguments. Whitespace separates
/// arguments, single and double quotes group characters (a backslash escapes inside double quotes
/// and outside of quotes), and `#` starts a comment at the beginning of a word.
pub fn split_args(text: &str) -> Result<Vec<String>> {
    let mut args = vec![];
    let mut current = String::new();
    let mut in_word = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '#' if !in_word => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            c if c.is_whitespace() => {
                if in_word {
                    args.push(std::mem::take(&mut current));
                    in_word = false;
                }
            }
            '\'' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(c) => current.push(c),
                        None => bail!("unterminated single quote"),
                    }
                }
            }
            '"' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') if matches!(chars.peek(), Some('"' | '\\')) => {
                            current.push(chars.next().unwrap());
                        }
                        Some(c) => current.push(c),
                        None => bail!("unterminated double quote"),
                    }
                }
            }
            // On Windows, backslashes are path separators and must stay.
            '\\' if !cfg!(target_os = "windows") => {
                in_word = true;
                match chars.next() {
                    Some(c) => current.push(c),
                    None => bail!("trailing backslash"),
                }
            }
            c => {
                in_word = true;
                current.push(c);
            }
        }
    }
    if in_word {
        args.push(current);
    }
    Ok(args)
}

/// This function reads an options file. A missing file is not an error.
pub fn read_args_file(path: &Path) -> Result<Option<Vec<String>>> {
    match read_to_string(path) {
        Ok(text) => split_args(&text)
            .map(Some)
            .with_context(|| format!("invalid options file {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => {
            Err(error).with_context(|| format!("could not read options file {}", path.display()))
        }
    }
}

/// This function returns the default location of the options file.
pub fn default_args_file() -> Option<PathBuf> {
    user_directory().map(|directory| directory.join(ARGS_FILE))
}

#[cfg(test)]
mod tests {
    use std::fs::{create_dir_all, write};

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gc_paths_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn splits_arguments() {
        assert_eq!(
            split_args("--interface en0 # comment\n--sync 'a b' \"c \\\" d\" e#f").unwrap(),
            vec!["--interface", "en0", "--sync", "a b", "c \" d", "e#f"]
        );
        assert!(split_args("'open").is_err());
        assert!(split_args("").unwrap().is_empty());
    }

    #[test]
    fn explicit_has_no_fallback() {
        let candidates = config_candidates(
            Some(Path::new("/x")),
            Some(Path::new("/u")),
            Path::new("/a/b/exe"),
        );
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].0, ConfigOrigin::Explicit);
    }

    #[test]
    fn user_directory_wins_over_development() {
        let root = temp_dir("order");
        let user = root.join("user");
        let exe = root
            .join("target")
            .join("release")
            .join("game_controller_app");
        let development = root.join("config");
        create_dir_all(&user).unwrap();
        create_dir_all(&development).unwrap();
        create_dir_all(exe.parent().unwrap()).unwrap();
        write(development.join(MARKER_FILE), "").unwrap();
        let resolved = resolve_config_directory(None, Some(&user), &exe).unwrap();
        assert_eq!(resolved.origin, ConfigOrigin::Development);
        write(user.join(MARKER_FILE), "").unwrap();
        let resolved = resolve_config_directory(None, Some(&user), &exe).unwrap();
        assert_eq!(resolved.origin, ConfigOrigin::User);
        assert!(stale_config_warning(&resolved).is_some());
    }

    #[test]
    fn error_lists_candidates() {
        let root = temp_dir("missing");
        let exe = root
            .join("target")
            .join("release")
            .join("game_controller_app");
        let error = resolve_config_directory(None, Some(&root.join("user")), &exe).unwrap_err();
        let message = format!("{error}");
        assert!(message.contains("(user)") && message.contains("(development)"));
    }
}
