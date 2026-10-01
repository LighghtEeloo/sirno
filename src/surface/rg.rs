//! Ripgrep integration and generated-footer preprocessing.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::surface::error::CommandError;
use crate::{GeneratedLinkBody, SirnoConfig};

const RG_PREPROCESSOR_ARGV0_PREFIX: &str = "sirno-rg-preprocess-";

pub(crate) fn rg_args_to_strings(args: Vec<OsString>) -> Result<Vec<String>, CommandError> {
    args.into_iter().map(|arg| arg.into_string().map_err(CommandError::RgArgumentNotUtf8)).collect()
}

pub(crate) fn resolve_lake_path_for_rg(
    lake_path: Option<&Path>, config_path: &Path,
) -> Result<PathBuf, CommandError> {
    if let Some(lake_path) = lake_path {
        return Ok(lake_path.to_path_buf());
    }

    let config = SirnoConfig::from_file(config_path)?;
    Ok(config.resolve_lake(config_path))
}

pub(crate) fn is_rg_preprocessor_invocation() -> bool {
    env::args_os()
        .next()
        .and_then(|arg| PathBuf::from(arg).file_name().map(|name| name.to_os_string()))
        .is_some_and(|name| name.to_string_lossy().starts_with(RG_PREPROCESSOR_ARGV0_PREFIX))
}

pub(crate) fn run_rg_preprocessor_from_env() -> Result<ExitCode, CommandError> {
    let mut args = env::args_os().skip(1);
    let Some(path) = args.next() else {
        return Err(CommandError::RgPreprocessorArgumentCount);
    };
    if args.next().is_some() {
        return Err(CommandError::RgPreprocessorArgumentCount);
    }

    run_rg_preprocessor(&PathBuf::from(path))
}

fn run_rg_preprocessor(path: &Path) -> Result<ExitCode, CommandError> {
    let body = fs::read_to_string(path).map_err(|source| {
        CommandError::ReadRgPreprocessorInput { path: path.to_path_buf(), source }
    })?;
    let masked = GeneratedLinkBody::new(&body).mask()?;
    io::stdout().write_all(masked.as_bytes()).map_err(CommandError::WriteRgPreprocessorOutput)?;
    Ok(ExitCode::SUCCESS)
}

#[derive(Debug)]
pub(crate) struct RgPreprocessorLink {
    path: PathBuf,
}

impl RgPreprocessorLink {
    pub(crate) fn create() -> Result<Self, CommandError> {
        let current_exe = env::current_exe().map_err(CommandError::LocateCurrentExe)?;
        let mut path = env::temp_dir();
        path.push(format!(
            "{RG_PREPROCESSOR_ARGV0_PREFIX}{}-{}",
            std::process::id(),
            current_time_nanos()
        ));
        #[cfg(not(unix))]
        if let Some(extension) = current_exe.extension() {
            path.set_extension(extension);
        }

        create_rg_preprocessor_invoker(&current_exe, &path).map_err(|source| {
            CommandError::CreateRgPreprocessorInvoker { path: path.clone(), source }
        })?;
        Ok(Self { path })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for RgPreprocessorLink {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn current_time_nanos() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()
}

#[cfg(unix)]
fn create_rg_preprocessor_invoker(current_exe: &Path, path: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(current_exe, path)
}

#[cfg(not(unix))]
fn create_rg_preprocessor_invoker(current_exe: &Path, path: &Path) -> io::Result<()> {
    fs::copy(current_exe, path).map(|_| ())
}
