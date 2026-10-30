pub mod host;
pub mod install;
pub mod transport;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
include!(concat!(env!("OUT_DIR"), "/payload.rs"));
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const LAYOUTS: &[&str] = &["main", "terminals", "win98"];
pub fn tools() -> Vec<Value> {
    serde_json::from_str(include_str!("../agent-tools.json")).expect("valid embedded tool manifest")
}
pub fn app_home() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("HERDR_APP_HOME").filter(|s| !s.is_empty()) {
        return Ok(p.into());
    }
    let root = if let Some(p) = std::env::var_os("XDG_CONFIG_HOME").filter(|s| !s.is_empty()) {
        PathBuf::from(p)
    } else {
        PathBuf::from(
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .context("Cannot determine home; set HERDR_APP_HOME")?,
        )
        .join(".config")
    };
    Ok(root.join("herdr-app"))
}
pub fn validate(tool: &Value, args: &Value) -> Result<()> {
    let args = args.as_object().context("arguments must be an object")?;
    let schema = &tool["inputSchema"];
    let properties = schema["properties"]
        .as_object()
        .context("invalid embedded tool schema")?;
    for key in schema["required"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if !args.contains_key(key) {
            bail!("missing argument: {key}");
        }
    }
    for (key, value) in args {
        let rule = properties
            .get(key)
            .with_context(|| format!("unknown argument: {key}"))?;
        let valid = match rule["type"].as_str() {
            Some("string") => value.is_string(),
            Some("boolean") => value.is_boolean(),
            _ => false,
        };
        if !valid
            || rule["enum"].as_array().is_some_and(|e| !e.contains(value))
            || (rule["minLength"].as_u64().unwrap_or(0) > 0
                && value.as_str().is_none_or(|s| s.trim().is_empty()))
        {
            bail!("invalid argument: {key}");
        }
    }
    Ok(())
}
pub struct Engine<H: host::Host> {
    pub home: PathBuf,
    pub host: H,
}
impl<H: host::Host> Engine<H> {
    pub fn call(&self, name: &str, args: &Value) -> Result<Value> {
        let tool = tools()
            .into_iter()
            .find(|t| t["name"] == name)
            .with_context(|| format!("unknown tool: {name}"))?;
        validate(&tool, args)?;
        match name {
            "herdr_auto_titles_install" => self.install(args),
            "herdr_auto_titles_uninstall" => self.uninstall(),
            "herdr_auto_titles_status" => self.status(),
            "herdr_auto_titles_enable" => self.enable(args["layout"].as_str().unwrap()),
            "herdr_auto_titles_disable" => self.disable(),
            "herdr_titles_list" => self.titles(),
            "herdr_tab_title_set" => {
                let result =
                    self.host
                        .run(&["do".into(), "tab.rename".into(), args.to_string()])?;
                Ok(
                    json!({"id":args["id"],"mode":if args["name"].as_str().unwrap().trim().is_empty(){"automatic"}else{"manual"},"result":result}),
                )
            }
            _ => bail!("unimplemented tool: {name}"),
        }
    }
    pub fn titles(&self) -> Result<Value> {
        let current = self.host.run(&["status".into()])?;
        let world = self.host.run(&["ls".into()])?;
        let shared: Value = serde_json::from_slice(&fs::read(self.home.join("shared.json"))?)?;
        let snapshot = &shared["auto-titles.snapshot"];
        if snapshot["data"].is_null()
            || !current["layout"]
                .as_str()
                .is_some_and(|s| s.starts_with("auto-titles-"))
            || snapshot["layout"] != current["layout"]
        {
            bail!(
                "Auto Titles is not active or has not rendered yet. Enable one of its layouts and retry."
            );
        }
        fn ids(spaces: &Value) -> Result<Vec<String>> {
            let mut ids = vec![];
            for s in spaces.as_array().context("invalid spaces snapshot")? {
                ids.push(s["id"].as_str().context("missing space ID")?.to_owned());
                for t in s["tabs"].as_array().context("invalid tabs snapshot")? {
                    ids.push(t["id"].as_str().context("missing tab ID")?.to_owned());
                }
            }
            ids.sort();
            Ok(ids)
        }
        if ids(&world["spaces"])? != ids(&snapshot["data"]["spaces"])? {
            bail!(
                "The title snapshot is catching up with the window. Retry after its next render."
            );
        }
        // Return the title projection only, never unrelated shared state.
        Ok(
            json!({"source":"last-rendered","updated_at":snapshot["updated_at"],"layout":snapshot["layout"],"data":snapshot["data"]}),
        )
    }
}
