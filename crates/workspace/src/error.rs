use std::fmt;
use std::io;
use std::path::PathBuf;

/// Why a folder could not become a project; each variant carries the path as resolved.
#[derive(Debug)]
pub enum AddProjectError {
    NotFound(PathBuf),
    NotADirectory(PathBuf),
    Io(PathBuf, io::Error),
}

impl fmt::Display for AddProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(path) => write!(f, "{} does not exist", path.display()),
            Self::NotADirectory(path) => write!(f, "{} is not a folder", path.display()),
            Self::Io(path, error) => write!(f, "cannot open {}: {error}", path.display()),
        }
    }
}

impl std::error::Error for AddProjectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(_, error) => Some(error),
            Self::NotFound(_) | Self::NotADirectory(_) => None,
        }
    }
}

/// Why the saved workspace could not be read.
#[derive(Debug)]
pub enum LoadError {
    Io(io::Error),
    Corrupt(serde_json::Error),
    /// Written by a newer build whose format this one does not know.
    UnsupportedVersion(u32),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "cannot read the workspace: {error}"),
            Self::Corrupt(error) => write!(f, "the workspace file is corrupt: {error}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "the workspace file has unsupported version {version}")
            }
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Corrupt(error) => Some(error),
            Self::UnsupportedVersion(_) => None,
        }
    }
}
