// SPITFIRE NG
// Preservation-driven modern cross-platform reimplementation of
// Buffalo Creek Software's SPITFIRE Bulletin Board System
//
// Copyright (c) 2026 Craig Daters and SPITFIRE NG contributors
// Licensed under MIT OR Apache-2.0
//
// This file is part of the SPITFIRE NG project.
// See the repository documentation for architecture, provenance,
// compatibility research, security, and contribution guidelines.

use std::fs;
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

use crate::config::{PathConfig, ValidatedConfig};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LogicalPath {
    System,
    Work,
    Display,
    Message,
    External,
}

impl LogicalPath {
    pub const ALL: [Self; 5] = [
        Self::System,
        Self::Work,
        Self::Display,
        Self::Message,
        Self::External,
    ];

    pub fn historical_name(self) -> &'static str {
        match self {
            Self::System => "SYSTEM",
            Self::Work => "WORK",
            Self::Display => "DISPLAY",
            Self::Message => "MESSAGE",
            Self::External => "EXTERNAL",
        }
    }
}

/// Resolved host paths kept behind stock SPITFIRE logical names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalPaths {
    root: PathBuf,
    system: PathBuf,
    work: PathBuf,
    display: PathBuf,
    message: PathBuf,
    external: PathBuf,
    database: PathBuf,
}

impl LogicalPaths {
    pub fn resolve(root: &Path, config: &ValidatedConfig) -> Result<Self, PathError> {
        let root = absolute_root(root)?;
        let PathConfig {
            system,
            work,
            display,
            message,
            external,
        } = &config.paths;

        let system = resolve_one(&root, "SYSTEM", system)?;
        let work = resolve_one(&root, "WORK", work)?;
        let display = resolve_one(&root, "DISPLAY", display)?;
        let message = resolve_one(&root, "MESSAGE", message)?;
        let external = resolve_one(&root, "EXTERNAL", external)?;
        let database = work.join(&config.database_file);

        Ok(Self {
            root,
            system,
            work,
            display,
            message,
            external,
            database,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn get(&self, logical: LogicalPath) -> &Path {
        match logical {
            LogicalPath::System => &self.system,
            LogicalPath::Work => &self.work,
            LogicalPath::Display => &self.display,
            LogicalPath::Message => &self.message,
            LogicalPath::External => &self.external,
        }
    }

    pub fn database(&self) -> &Path {
        &self.database
    }

    pub fn create_directories(&self) -> Result<(), PathError> {
        for logical in LogicalPath::ALL {
            let path = self.get(logical);
            fs::create_dir_all(path).map_err(|source| PathError::Create {
                logical: logical.historical_name(),
                path: path.to_path_buf(),
                source,
            })?;
        }
        Ok(())
    }
}

pub(crate) fn validate_configured_path(
    logical: &'static str,
    path: &Path,
) -> Result<(), PathError> {
    if path.as_os_str().is_empty() {
        return Err(PathError::Empty { logical });
    }
    if !path.is_absolute()
        && path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(PathError::ParentTraversal {
            logical,
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

fn resolve_one(
    root: &Path,
    logical: &'static str,
    configured: &Path,
) -> Result<PathBuf, PathError> {
    validate_configured_path(logical, configured)?;
    if configured.is_absolute() {
        Ok(configured.to_path_buf())
    } else {
        Ok(root.join(configured))
    }
}

fn absolute_root(root: &Path) -> Result<PathBuf, PathError> {
    if root.is_absolute() {
        return Ok(root.to_path_buf());
    }
    std::env::current_dir()
        .map(|current| current.join(root))
        .map_err(PathError::CurrentDirectory)
}

#[derive(Debug, Error)]
pub enum PathError {
    #[error("logical {logical} path must not be empty")]
    Empty { logical: &'static str },
    #[error("relative logical {logical} path must not contain '..': {path}")]
    ParentTraversal {
        logical: &'static str,
        path: PathBuf,
    },
    #[error("could not determine the current directory: {0}")]
    CurrentDirectory(#[source] std::io::Error),
    #[error("could not create logical {logical} directory {path}: {source}")]
    Create {
        logical: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuntimeConfig;

    #[test]
    fn resolves_all_stock_logical_paths_without_leaking_config_paths() {
        let config = RuntimeConfig::synthetic_fixture().validate().unwrap();
        let root = Path::new("/fixture-root");
        let paths = LogicalPaths::resolve(root, &config).unwrap();

        assert_eq!(paths.get(LogicalPath::System), root.join("system"));
        assert_eq!(paths.get(LogicalPath::Work), root.join("work"));
        assert_eq!(paths.get(LogicalPath::Display), root.join("display"));
        assert_eq!(paths.get(LogicalPath::Message), root.join("message"));
        assert_eq!(paths.get(LogicalPath::External), root.join("external"));
        assert_eq!(paths.database(), root.join("work/spitfire-ng.sqlite3"));
    }

    #[test]
    fn rejects_relative_parent_traversal() {
        let mut config = RuntimeConfig::synthetic_fixture();
        config.paths.display = PathBuf::from("display/../../outside");
        assert!(matches!(
            config.validate(),
            Err(crate::ConfigError::InvalidPath(
                PathError::ParentTraversal {
                    logical: "DISPLAY",
                    ..
                }
            ))
        ));
    }

    #[test]
    fn creates_each_resolved_directory() {
        let temp = tempfile::tempdir().unwrap();
        let config = RuntimeConfig::synthetic_fixture().validate().unwrap();
        let paths = LogicalPaths::resolve(temp.path(), &config).unwrap();
        paths.create_directories().unwrap();
        for logical in LogicalPath::ALL {
            assert!(paths.get(logical).is_dir());
        }
    }
}
