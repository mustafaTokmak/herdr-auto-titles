use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::{
    ffi::OsString,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};
use wait_timeout::ChildExt;

pub trait Host {
    fn run(&self, args: &[String]) -> Result<Value>;
    fn check(&self) -> Result<()>;
}
pub struct SystemHost {
    pub executable: OsString,
    pub home: PathBuf,
}
impl SystemHost {
    pub fn new(home: PathBuf) -> Self {
        Self {
            executable: std::env::var_os("HERDR_APP_BIN").unwrap_or_else(|| "herdr-app".into()),
            home,
        }
    }
    fn output(&self, args: &[String], home: &Path) -> Result<String> {
        let mut child = Command::new(&self.executable)
            .args(args)
            .env("HERDR_APP_HOME", home)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| {
                format!(
                    "Cannot run {:?}; set HERDR_APP_BIN to the Herdr executable",
                    self.executable
                )
            })?;
        fn reader<R: Read + Send + 'static>(
            stream: R,
        ) -> thread::JoinHandle<std::io::Result<Vec<u8>>> {
            thread::spawn(move || {
                let mut bytes = vec![];
                stream.take(8 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
                Ok(bytes)
            })
        }
        let stdout = reader(child.stdout.take().unwrap());
        let stderr = reader(child.stderr.take().unwrap());
        let status = match child.wait_timeout(Duration::from_secs(15))? {
            Some(status) => status,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                bail!("Herdr command timed out after 15 seconds");
            }
        };
        let out = stdout
            .join()
            .map_err(|_| anyhow::anyhow!("stdout reader failed"))??;
        let err = stderr
            .join()
            .map_err(|_| anyhow::anyhow!("stderr reader failed"))??;
        if out.len() > 8 * 1024 * 1024 || err.len() > 8 * 1024 * 1024 {
            bail!("Herdr output exceeded 8 MiB");
        }
        if !status.success() {
            bail!(
                "Herdr command failed: {}",
                String::from_utf8_lossy(&err).trim()
            );
        }
        Ok(String::from_utf8(out)?)
    }
}
impl Host for SystemHost {
    fn run(&self, args: &[String]) -> Result<Value> {
        let value: Value = serde_json::from_str(&self.output(args, &self.home)?)?;
        if let Some(error) = value.get("error") {
            bail!("Herdr: {error}");
        }
        Ok(value)
    }
    fn check(&self) -> Result<()> {
        let version = self.output(&["--version".into()], &self.home)?;
        let numbers = version
            .split_whitespace()
            .find_map(|s| {
                let n: Vec<_> = s
                    .trim_start_matches('v')
                    .split('.')
                    .map(str::parse::<u32>)
                    .collect::<std::result::Result<_, _>>()
                    .ok()?;
                (n.len() == 3).then_some(n)
            })
            .context("Cannot determine Herdr version")?;
        if numbers.as_slice() < [0, 1, 9].as_slice() {
            bail!("Herdr desktop 0.1.9 or newer is required");
        }
        let temp = tempfile::tempdir()?;
        let home = temp.path().join("check-home");
        fs::create_dir(&home)?;
        for (name, bytes) in crate::PAYLOAD {
            let path = temp.path().join(name);
            fs::create_dir_all(path.parent().unwrap())?;
            fs::write(path, bytes)?;
        }
        for entry in fs::read_dir(temp.path().join("mods"))? {
            let path = entry?.path();
            self.output(
                &["check".into(), path.to_string_lossy().into_owned()],
                &home,
            )?;
        }
        Ok(())
    }
}
