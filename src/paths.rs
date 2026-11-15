use std::env;
use std::path::PathBuf;

/// Filesystem locations resolved once from the environment, passed explicitly so
/// detection can run against fixture trees in tests.
#[derive(Debug, Clone)]
pub struct Paths {
    pub home: PathBuf,
    pub config_home: PathBuf,
    pub data_home: PathBuf,
    pub state_home: PathBuf,
    pub data_dirs: Vec<PathBuf>,
    pub path_dirs: Vec<PathBuf>,
}

impl Paths {
    pub fn from_env() -> Self {
        let home = env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        let config_home = non_empty_var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        let data_home = non_empty_var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"));
        let state_home = non_empty_var("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state"));
        let data_dirs = non_empty_var("XDG_DATA_DIRS")
            .unwrap_or_else(|| "/usr/local/share:/usr/share".to_string())
            .split(':')
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect();
        let path_dirs = env::var_os("PATH")
            .map(|p| env::split_paths(&p).collect())
            .unwrap_or_default();
        Self {
            home,
            config_home,
            data_home,
            state_home,
            data_dirs,
            path_dirs,
        }
    }

    /// Desktop entry directories, highest precedence first.
    pub fn application_dirs(&self) -> Vec<PathBuf> {
        std::iter::once(&self.data_home)
            .chain(&self.data_dirs)
            .map(|d| d.join("applications"))
            .collect()
    }

    /// Resolves a bare program name through `PATH`; paths with a `/` are returned as is.
    pub fn which(&self, program: &str) -> Option<PathBuf> {
        if program.contains('/') {
            return Some(PathBuf::from(program));
        }
        self.path_dirs
            .iter()
            .map(|d| d.join(program))
            .find(|p| p.is_file())
    }
}

fn non_empty_var(key: &str) -> Option<String> {
    env::var(key).ok().filter(|v| !v.is_empty())
}
