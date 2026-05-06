// Derived from openai/codex commit 6e838a19fa
//   path: codex-rs/utils/absolute-path/src/lib.rs
//   path: codex-rs/utils/absolute-path/src/absolutize.rs
// SPDX-License-Identifier: Apache-2.0
//
// `absolutize` submodule is itself adapted from path-absolutize 3.1.1
// (Copyright (c) 2018 magiclen.org / Ron Li, MIT-licensed) — see upstream
// commentary preserved at the top of the inlined `absolutize` module.

//! Slim port of `codex-utils-absolute-path::AbsolutePathBuf`.
//!
//! Per V'-b "port-on-demand" rule, this module **omits** the upstream surface
//! that `windows-sandbox-rs` does not consume:
//!
//! - `JsonSchema` / `ts_rs::TS` derives (no schema generation in this crate).
//! - `AbsolutePathBufGuard` thread-local + custom `Deserialize` impl (we use
//!   the default derive, which only accepts already-absolute paths — sufficient
//!   for the `WritableRoot` payload deserialized from sandbox configs).
//! - `canonicalize_preserving_symlinks` /
//!   `canonicalize_existing_preserving_symlinks` (used elsewhere in codex CLI,
//!   not by the windows sandbox).
//! - `test_support::{PathExt, PathBufExt}` extension traits (depend on
//!   `expect()` and are unused on this path).
//!
//! The retained surface (`from_absolute_path`, `from_absolute_path_checked`,
//! `try_from`, `resolve_path_against_base`, `join`, `parent`, `ancestors`,
//! `as_path`, `into_path_buf`, `to_path_buf`, `to_string_lossy`, `display`,
//! `current_dir`, `relative_to_current_dir`, `canonicalize`, plus `Deref` /
//! `AsRef<Path>` / `From<AbsolutePathBuf> for PathBuf`) is a verbatim copy of
//! the upstream definitions — only the noise above is dropped.

use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::de::Error as SerdeError;
use std::path::Display;
use std::path::Path;
use std::path::PathBuf;

mod absolutize;

/// A path that is guaranteed to be absolute and normalized (though it is not
/// guaranteed to be canonicalized or exist on the filesystem).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(into = "PathBuf")]
pub struct AbsolutePathBuf(PathBuf);

impl AbsolutePathBuf {
    fn maybe_expand_home_directory(path: &Path) -> PathBuf {
        if let Some(path_str) = path.to_str()
            && let Some(home) = dirs::home_dir()
            && let Some(rest) = path_str.strip_prefix('~')
        {
            if rest.is_empty() {
                return home;
            } else if let Some(rest) = rest.strip_prefix('/') {
                return home.join(rest.trim_start_matches('/'));
            } else if cfg!(windows)
                && let Some(rest) = rest.strip_prefix('\\')
            {
                return home.join(rest.trim_start_matches('\\'));
            }
        }
        path.to_path_buf()
    }

    pub fn resolve_path_against_base<P: AsRef<Path>, B: AsRef<Path>>(
        path: P,
        base_path: B,
    ) -> Self {
        let expanded = Self::maybe_expand_home_directory(path.as_ref());
        Self(absolutize::absolutize_from(&expanded, base_path.as_ref()))
    }

    pub fn from_absolute_path<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let expanded = Self::maybe_expand_home_directory(path.as_ref());
        Ok(Self(absolutize::absolutize(&expanded)?))
    }

    pub fn from_absolute_path_checked<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let expanded = Self::maybe_expand_home_directory(path.as_ref());
        if !expanded.is_absolute() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("path is not absolute: {}", path.as_ref().display()),
            ));
        }

        Ok(Self(absolutize::absolutize_from(&expanded, Path::new("/"))))
    }

    pub fn current_dir() -> std::io::Result<Self> {
        let current_dir = std::env::current_dir()?;
        Ok(Self(absolutize::absolutize_from(
            &current_dir,
            &current_dir,
        )))
    }

    /// Construct an absolute path from `path`, resolving relative paths against
    /// the process current working directory.
    pub fn relative_to_current_dir<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        Ok(Self::resolve_path_against_base(
            path,
            std::env::current_dir()?,
        ))
    }

    pub fn join<P: AsRef<Path>>(&self, path: P) -> Self {
        Self::resolve_path_against_base(path, &self.0)
    }

    pub fn canonicalize(&self) -> std::io::Result<Self> {
        dunce::canonicalize(&self.0).map(Self)
    }

    pub fn parent(&self) -> Option<Self> {
        self.0.parent().map(|p| {
            debug_assert!(
                p.is_absolute(),
                "parent of AbsolutePathBuf must be absolute"
            );
            Self(p.to_path_buf())
        })
    }

    pub fn ancestors(&self) -> impl Iterator<Item = Self> + '_ {
        self.0.ancestors().map(|p| {
            debug_assert!(
                p.is_absolute(),
                "ancestor of AbsolutePathBuf must be absolute"
            );
            Self(p.to_path_buf())
        })
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn into_path_buf(self) -> PathBuf {
        self.0
    }

    pub fn to_path_buf(&self) -> PathBuf {
        self.0.clone()
    }

    pub fn to_string_lossy(&self) -> std::borrow::Cow<'_, str> {
        self.0.to_string_lossy()
    }

    pub fn display(&self) -> Display<'_> {
        self.0.display()
    }
}

impl AsRef<Path> for AbsolutePathBuf {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl std::ops::Deref for AbsolutePathBuf {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<AbsolutePathBuf> for PathBuf {
    fn from(path: AbsolutePathBuf) -> Self {
        path.into_path_buf()
    }
}

impl TryFrom<&Path> for AbsolutePathBuf {
    type Error = std::io::Error;

    fn try_from(value: &Path) -> Result<Self, Self::Error> {
        Self::from_absolute_path(value)
    }
}

impl TryFrom<PathBuf> for AbsolutePathBuf {
    type Error = std::io::Error;

    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        Self::from_absolute_path(value)
    }
}

impl TryFrom<&str> for AbsolutePathBuf {
    type Error = std::io::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_absolute_path(value)
    }
}

impl TryFrom<String> for AbsolutePathBuf {
    type Error = std::io::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_absolute_path(value)
    }
}

/// Deserialize an absolute path. Mirrors upstream's "no guard installed"
/// branch: only already-absolute paths are accepted; relative paths produce
/// a deserialization error. The `AbsolutePathBufGuard` thread-local mechanism
/// upstream uses to relative-resolve during config load is intentionally not
/// ported (windows-sandbox configs are loaded through paths that are already
/// absolute by construction).
impl<'de> Deserialize<'de> for AbsolutePathBuf {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let path = PathBuf::deserialize(deserializer)?;
        if path.is_absolute() {
            Self::from_absolute_path(path).map_err(SerdeError::custom)
        } else {
            Err(SerdeError::custom("AbsolutePathBuf: path is not absolute"))
        }
    }
}

/// Helper to construct platform-absolute test paths.
///
/// On Windows, `/tmp/example` maps to `C:\tmp\example`. Lifted from the
/// upstream `test_support` module; the `PathExt`/`PathBufExt` traits were
/// dropped because they require `expect()` (forbidden by the workspace
/// no-panic guard).
#[cfg(test)]
pub(crate) fn test_path_buf(unix_path: &str) -> PathBuf {
    if cfg!(windows) {
        let mut path = PathBuf::from(r"C:\");
        path.extend(
            unix_path
                .trim_start_matches('/')
                .split('/')
                .filter(|segment| !segment.is_empty()),
        );
        path
    } else {
        PathBuf::from(unix_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn from_absolute_path_checked_rejects_relative_path() {
        let err = AbsolutePathBuf::from_absolute_path_checked("relative/path")
            .expect_err("relative path should fail");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn resolve_relative_path_against_base() {
        let base = test_path_buf("/base");
        let abs = AbsolutePathBuf::resolve_path_against_base("file.txt", &base);
        assert_eq!(abs.as_path(), base.join("file.txt").as_path());
    }

    #[test]
    fn resolve_normalizes_dot_dot() {
        let base = test_path_buf("/base");
        let abs = AbsolutePathBuf::resolve_path_against_base("./nested/../file.txt", &base);
        assert_eq!(abs.as_path(), base.join("file.txt").as_path());
    }

    #[test]
    fn ancestors_returns_absolute_path_bufs() {
        let abs = AbsolutePathBuf::from_absolute_path_checked(test_path_buf("/tmp/one/two"))
            .expect("absolute path");
        let got: Vec<PathBuf> = abs.ancestors().map(|p| p.to_path_buf()).collect();
        let expected = vec![
            test_path_buf("/tmp/one/two"),
            test_path_buf("/tmp/one"),
            test_path_buf("/tmp"),
            test_path_buf("/"),
        ];
        assert_eq!(got, expected);
    }

    #[test]
    fn try_from_absolute_str_succeeds() {
        let s = test_path_buf("/abs/path");
        let abs = AbsolutePathBuf::try_from(s.to_str().expect("utf8")).expect("absolute");
        assert_eq!(abs.as_path(), s.as_path());
    }

    #[test]
    fn serde_round_trips_absolute_path() {
        let s = test_path_buf("/abs/path");
        let abs =
            AbsolutePathBuf::from_absolute_path_checked(&s).expect("absolute path constructable");
        let json = serde_json::to_string(&abs).expect("serialize");
        let back: AbsolutePathBuf = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.as_path(), s.as_path());
    }

    #[test]
    fn serde_rejects_relative_path() {
        let json = serde_json::to_string("relative/path").expect("serialize string");
        let err = serde_json::from_str::<AbsolutePathBuf>(&json)
            .expect_err("relative path should fail");
        assert!(err.to_string().contains("not absolute"));
    }
}
