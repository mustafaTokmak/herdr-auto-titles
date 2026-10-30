use crate::{Engine, LAYOUTS, PAYLOAD, VERSION, host::Host};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};
const RECEIPT: &str = "data/herdr-auto-titles/installation.json";
pub type KeyBindings = BTreeMap<String, BTreeMap<String, String>>;
type KeyState = (Option<Vec<u8>>, KeyBindings);
const WIN98: &str = "layouts/auto-titles-win98.json";
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub name: String,
    pub version: String,
    pub files: BTreeMap<String, String>,
    #[serde(default)]
    pub previous_layout: String,
    #[serde(default)]
    pub key_bindings: KeyBindings,
    #[serde(default)]
    pub runtime_layouts: BTreeMap<String, String>,
}
fn metadata(path: &Path) -> Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(m) => Ok(Some(m)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
fn is_link(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}
pub fn safe_path(home: &Path, rel: &str) -> Result<PathBuf> {
    if rel.contains(['\\', ':'])
        || rel
            .split('/')
            .any(|p| p.is_empty() || p == ".." || p == ".")
    {
        bail!("Unsafe managed path: {rel}");
    }
    let mut path = home.to_owned();
    for part in rel.split('/') {
        path.push(part);
        if metadata(&path)?.is_some_and(|m| is_link(&m)) {
            bail!("Refusing symlink: {}", path.display());
        }
    }
    Ok(path)
}
fn namespace(s: &str) -> bool {
    s.strip_prefix("auto-titles-").is_some_and(|p| {
        !p.is_empty()
            && p.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    })
}
fn owned(rel: &str) -> bool {
    let p: Vec<_> = rel.split('/').collect();
    match p.as_slice() {
        ["mods", name, rest @ ..] => namespace(name) && !rest.is_empty(),
        ["layouts", name] => [".json", ".svg"]
            .iter()
            .any(|ext| name.strip_suffix(ext).is_some_and(namespace)),
        _ => false,
    }
}
fn read_receipt(home: &Path) -> Result<Option<Receipt>> {
    let path = safe_path(home, RECEIPT)?;
    if metadata(&path)?.is_none() {
        return Ok(None);
    }
    let receipt: Receipt =
        serde_json::from_slice(&fs::read(path)?).context("Invalid installation receipt")?;
    if receipt.name != "herdr-auto-titles" {
        bail!("Invalid installation receipt");
    }
    for rel in receipt.files.keys() {
        if !owned(rel) {
            bail!("Unowned receipt path: {rel}");
        }
        safe_path(home, rel)?;
    }
    for section in receipt.key_bindings.keys() {
        if !section.strip_prefix("layout:").is_some_and(namespace) {
            bail!("Unowned key section in receipt");
        }
    }
    Ok(Some(receipt))
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::create_dir_all(path.parent().context("Missing parent directory")?)?;
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}
fn apply(path: &Path, bytes: &Option<Vec<u8>>) -> Result<()> {
    match bytes {
        Some(b) => atomic(path, b),
        None => match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        },
    }
}
fn transaction(home: &Path, changes: &[(String, Option<Vec<u8>>)]) -> Result<()> {
    let mut before = vec![];
    for (rel, _) in changes {
        let path = safe_path(home, rel)?;
        let bytes = match metadata(&path)? {
            Some(m) if !m.is_file() => bail!("Not a regular file: {}", path.display()),
            Some(_) => Some(fs::read(path)?),
            None => None,
        };
        before.push(bytes);
    }
    for (index, (rel, bytes)) in changes.iter().enumerate() {
        let result = safe_path(home, rel).and_then(|p| apply(&p, bytes));
        if let Err(error) = result {
            let mut rollback_errors = vec![];
            for i in (0..=index).rev() {
                if let Err(e) = safe_path(home, &changes[i].0).and_then(|p| apply(&p, &before[i])) {
                    rollback_errors.push(e.to_string());
                }
            }
            if !rollback_errors.is_empty() {
                bail!("{error}; rollback failed: {}", rollback_errors.join("; "));
            }
            return Err(error);
        }
    }
    Ok(())
}
fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}
fn node<'a>(value: &'a Value, id: &str) -> Option<&'a Value> {
    if value["id"] == id {
        return Some(value);
    }
    value["children"]
        .as_array()?
        .iter()
        .find_map(|n| node(n, id))
}
fn node_mut<'a>(value: &'a mut Value, id: &str) -> Option<&'a mut Value> {
    if value["id"] == id {
        return Some(value);
    }
    value
        .get_mut("children")?
        .as_array_mut()?
        .iter_mut()
        .find_map(|n| node_mut(n, id))
}
pub fn is_auto_fit(actual: &[u8], expected: &str) -> bool {
    fn check(actual: &[u8], expected: &str) -> Option<bool> {
        let mut now: Value = serde_json::from_slice(actual).ok()?;
        let mut base: Value = serde_json::from_str(expected).ok()?;
        let k = node(&now, "taskbar")?["size"]["px"].as_f64()? / 28.;
        if !k.is_finite() || k <= 0. {
            return Some(false);
        }
        let want = |n: f64| (n * k * 1000.).round() / 1000.;
        let near = |v: &Value, n: f64| v.as_f64().is_some_and(|a| (a - n).abs() < 0.002);
        let pane = &node(&now, "main")?["content"]["pane"];
        if !near(&node(&now, "desktop")?["size"]["px"], want(84.))
            || !near(&pane["header"]["height"], want(18.))
            || !near(&pane["border"]["width"], want(3.))
            || !near(&pane["focus"]["width"], want(3.))
            || !(near(&now["font_size"], 16.) || near(&now["font_size"], want(16.)))
        {
            return Some(false);
        }
        for layout in [&mut now, &mut base] {
            layout.as_object_mut()?.remove("font_size");
            for id in ["desktop", "taskbar"] {
                node_mut(layout, id)?["size"].as_object_mut()?.remove("px");
            }
            let pane = &mut node_mut(layout, "main")?["content"]["pane"];
            pane["header"].as_object_mut()?.remove("height");
            pane["border"].as_object_mut()?.remove("width");
            pane["focus"].as_object_mut()?.remove("width");
        }
        // serde_json distinguishes 10 from 10.0, but the JS host does not.
        fn equal(a: &Value, b: &Value) -> bool {
            match (a, b) {
                (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
                (Value::Array(a), Value::Array(b)) => {
                    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equal(a, b))
                }
                (Value::Object(a), Value::Object(b)) => {
                    a.len() == b.len()
                        && a.iter().all(|(k, a)| b.get(k).is_some_and(|b| equal(a, b)))
                }
                _ => a == b,
            }
        }
        Some(equal(&now, &base))
    }
    check(actual, expected).unwrap_or(false)
}
fn verify_owned(home: &Path, receipt: Option<&Receipt>) -> Result<()> {
    if let Some(receipt) = receipt {
        for (rel, expected) in &receipt.files {
            let path = safe_path(home, rel)?;
            let Some(info) = metadata(&path)? else {
                continue;
            };
            if info.is_file() {
                let bytes = fs::read(&path)?;
                if hash(&bytes) == *expected {
                    continue;
                }
                if rel == WIN98 {
                    let bundled = PAYLOAD.iter().find(|(p, _)| *p == WIN98).unwrap().1;
                    let baseline = receipt
                        .runtime_layouts
                        .get(rel)
                        .map(String::as_str)
                        .or_else(|| {
                            (hash(bundled) == *expected)
                                .then(|| std::str::from_utf8(bundled).unwrap())
                        });
                    if baseline.is_some_and(|b| is_auto_fit(&bytes, b)) {
                        continue;
                    }
                }
            }
            bail!(
                "Locally edited plugin file: {}. Save your edits elsewhere before updating or uninstalling.",
                path.display()
            );
        }
    }
    Ok(())
}
fn picker_keys(home: &Path, old: Option<&Receipt>, removing: bool) -> Result<KeyState> {
    let path = safe_path(home, "keys.json")?;
    let mut bindings = old.map(|r| r.key_bindings.clone()).unwrap_or_default();
    if metadata(&path)?.is_none() {
        return Ok((None, bindings));
    }
    let mut keys: Value = serde_json::from_slice(&fs::read(path)?)?;
    let object = keys
        .as_object_mut()
        .context("keys.json must be an object")?;
    if removing {
        for (section, entries) in &bindings {
            if let Some(current) = object.get_mut(section).and_then(Value::as_object_mut) {
                for (key, action) in entries {
                    if current.get(key).is_some_and(|v| v == action) {
                        current.remove(key);
                    }
                }
                if current.is_empty() {
                    object.remove(section);
                }
            }
        }
    } else {
        let global = object
            .get("global")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for layout in LAYOUTS {
            let section = format!("layout:auto-titles-{layout}");
            for (key, action) in &global {
                if action != "mod.open:picker" {
                    continue;
                }
                let entries = object
                    .entry(section.clone())
                    .or_insert_with(|| json!({}))
                    .as_object_mut()
                    .context("key section must be an object")?;
                if entries.contains_key(key) {
                    continue;
                }
                let target = "mod.open:auto-titles-picker";
                entries.insert(key.clone(), json!(target));
                bindings
                    .entry(section.clone())
                    .or_default()
                    .insert(key.clone(), target.into());
            }
        }
    }
    let bytes = if bindings.is_empty() {
        None
    } else {
        Some(encode(&keys)?)
    };
    Ok((bytes, bindings))
}
impl<H: Host> Engine<H> {
    fn lock(&self) -> Result<File> {
        let path = safe_path(&self.home, "data/herdr-auto-titles/operation.lock")?;
        fs::create_dir_all(path.parent().unwrap())?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        fs2::FileExt::try_lock_exclusive(&file)
            .context("Another Auto Titles operation is running")?;
        Ok(file)
    }
    pub fn status(&self) -> Result<Value> {
        let receipt = read_receipt(&self.home)?;
        let (app, error) = match self.host.run(&["status".into()]) {
            Ok(v) => (v, Value::Null),
            Err(e) => (Value::Null, json!(e.to_string())),
        };
        Ok(
            json!({"installed":receipt.is_some(),"version":receipt.map(|r|r.version),"home":self.home,"active":app["layout"].as_str().is_some_and(|s|s.starts_with("auto-titles-")),"layout":app["layout"],"app_error":error}),
        )
    }
    pub fn enable(&self, layout: &str) -> Result<Value> {
        let _lock = self.lock()?;
        self.enable_inner(layout)
    }
    fn enable_inner(&self, layout: &str) -> Result<Value> {
        if !LAYOUTS.contains(&layout) {
            bail!("layout must be one of {}", LAYOUTS.join(", "));
        }
        let mut receipt = read_receipt(&self.home)?.context("Install herdr-auto-titles first")?;
        let current = self.host.run(&["status".into()])?;
        if !current["layout"]
            .as_str()
            .is_some_and(|s| s.starts_with("auto-titles-"))
        {
            receipt.previous_layout = current["layout"].as_str().unwrap_or("main").into();
            transaction(&self.home, &[(RECEIPT.into(), Some(encode(&receipt)?))])?;
        }
        let name = format!("auto-titles-{layout}");
        self.host.run(&[
            "do".into(),
            "layout.switch".into(),
            json!({"name":name}).to_string(),
        ])?;
        Ok(json!({"active":true,"layout":name}))
    }
    pub fn disable(&self) -> Result<Value> {
        let _lock = self.lock()?;
        self.disable_inner()
    }
    fn disable_inner(&self) -> Result<Value> {
        let receipt = read_receipt(&self.home)?;
        let current = self.host.run(&["status".into()])?;
        if !current["layout"]
            .as_str()
            .is_some_and(|s| s.starts_with("auto-titles-"))
        {
            return Ok(json!({"active":false,"layout":current["layout"]}));
        }
        let target = receipt
            .as_ref()
            .map(|r| r.previous_layout.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("main");
        let layout = if safe_path(&self.home, &format!("layouts/{target}.json"))?.is_file() {
            target
        } else {
            "main"
        };
        self.host.run(&[
            "do".into(),
            "layout.switch".into(),
            json!({"name":layout}).to_string(),
        ])?;
        Ok(json!({"active":false,"layout":layout}))
    }
    pub fn install(&self, options: &Value) -> Result<Value> {
        if options
            .get("layout")
            .is_some_and(|v| !v.as_str().is_some_and(|s| LAYOUTS.contains(&s)))
        {
            bail!("Unknown layout");
        }
        if !self.home.join("mods").is_dir() || !self.home.join("layouts").is_dir() {
            bail!("Open the Herdr desktop app once before installing");
        }
        let _lock = self.lock()?;
        let old = read_receipt(&self.home)?;
        let preflight = || -> Result<()> {
            verify_owned(&self.home, old.as_ref())?;
            for (rel, _) in PAYLOAD {
                let path = safe_path(&self.home, rel)?;
                if metadata(&path)?.is_some()
                    && !old.as_ref().is_some_and(|r| r.files.contains_key(*rel))
                {
                    bail!(
                        "Destination is not owned by this plugin: {}",
                        path.display()
                    );
                }
            }
            Ok(())
        };
        preflight()?;
        self.host.check()?;
        preflight()?;
        let (keys, bindings) = picker_keys(&self.home, old.as_ref(), false)?;
        let receipt = Receipt {
            name: "herdr-auto-titles".into(),
            version: VERSION.into(),
            previous_layout: old
                .as_ref()
                .map(|r| r.previous_layout.clone())
                .unwrap_or_else(|| "main".into()),
            key_bindings: bindings,
            files: PAYLOAD
                .iter()
                .map(|(p, b)| (p.to_string(), hash(b)))
                .collect(),
            runtime_layouts: BTreeMap::from([(
                WIN98.into(),
                std::str::from_utf8(PAYLOAD.iter().find(|(p, _)| *p == WIN98).unwrap().1)?.into(),
            )]),
        };
        let mut changes: Vec<_> = PAYLOAD
            .iter()
            .map(|(p, b)| (p.to_string(), Some(b.to_vec())))
            .collect();
        changes.sort_by_key(|(p, _)| p.starts_with("layouts/"));
        if let Some(keys) = keys {
            changes.push(("keys.json".into(), Some(keys)));
        }
        if let Some(old) = &old {
            for rel in old.files.keys() {
                if !receipt.files.contains_key(rel) {
                    changes.push((rel.clone(), None));
                }
            }
        }
        changes.push((RECEIPT.into(), Some(encode(&receipt)?)));
        transaction(&self.home, &changes)?;
        let activation = if options["activate"] == false {
            Value::Null
        } else {
            match self.enable_inner(options["layout"].as_str().unwrap_or("main")) {
                Ok(v) => v,
                Err(e) => {
                    json!({"active":false,"error":e.to_string(),"hint":"Files installed. Start Herdr, then run enable."})
                }
            }
        };
        Ok(
            json!({"installed":true,"version":VERSION,"home":self.home,"updated":old.is_some(),"files":PAYLOAD.len(),"activation":activation}),
        )
    }
    pub fn uninstall(&self) -> Result<Value> {
        let _lock = self.lock()?;
        let Some(receipt) = read_receipt(&self.home)? else {
            return Ok(json!({"installed":false,"removed":0}));
        };
        verify_owned(&self.home, Some(&receipt))?;
        if self.status()?["active"] == true {
            self.disable_inner()?;
        }
        let (keys, _) = picker_keys(&self.home, Some(&receipt), true)?;
        let mut changes: Vec<_> = receipt.files.keys().map(|p| (p.clone(), None)).collect();
        if let Some(keys) = keys {
            changes.push(("keys.json".into(), Some(keys)));
        }
        changes.push((RECEIPT.into(), None));
        transaction(&self.home, &changes)?;
        let mut parents = BTreeSet::new();
        for (rel, _) in &changes {
            let mut dir = Path::new(rel).parent();
            while let Some(p) = dir {
                if p.as_os_str().is_empty()
                    || ["mods", "layouts", "data"]
                        .iter()
                        .any(|s| p == Path::new(s))
                {
                    break;
                }
                parents.insert(p.to_owned());
                dir = p.parent();
            }
        }
        let mut parents: Vec<_> = parents.into_iter().collect();
        parents.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
        for parent in parents {
            match fs::remove_dir(self.home.join(parent)) {
                Ok(()) => (),
                Err(e)
                    if [ErrorKind::NotFound, ErrorKind::DirectoryNotEmpty].contains(&e.kind()) => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(json!({"installed":false,"removed":receipt.files.len()}))
    }
}
