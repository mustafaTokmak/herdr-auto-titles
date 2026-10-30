use anyhow::{Result, bail};
use herdr_auto_titles::{
    Engine, PAYLOAD, VERSION,
    host::Host,
    install::{is_auto_fit, safe_path},
    tools, transport, validate,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    fs,
    path::PathBuf,
};
#[derive(Default)]
struct FakeHost {
    layout: RefCell<String>,
    offline: Cell<bool>,
    checks: Cell<usize>,
    fail_check: Cell<bool>,
    calls: RefCell<Vec<Vec<String>>>,
    on_check: RefCell<Option<Box<dyn Fn()>>>,
}
impl Host for FakeHost {
    fn run(&self, args: &[String]) -> Result<Value> {
        self.calls.borrow_mut().push(args.to_vec());
        if self.offline.get() {
            bail!("window closed");
        }
        match args[0].as_str() {
            "status" => Ok(json!({"layout":*self.layout.borrow()})),
            "ls" => Ok(json!({"spaces":[{"id":"s1","tabs":[{"id":"remote/tb1"}]}]})),
            "do" => {
                if args[1] == "layout.switch" {
                    *self.layout.borrow_mut() = serde_json::from_str::<Value>(&args[2])?["name"]
                        .as_str()
                        .unwrap()
                        .into();
                }
                Ok(json!({"ok":true}))
            }
            _ => bail!("unexpected host command"),
        }
    }
    fn check(&self) -> Result<()> {
        self.checks.set(self.checks.get() + 1);
        if self.fail_check.get() {
            bail!("invalid mod");
        }
        if let Some(f) = self.on_check.borrow().as_ref() {
            f();
        }
        Ok(())
    }
}
struct Fixture {
    _dir: tempfile::TempDir,
    engine: Engine<FakeHost>,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().to_owned();
        for p in ["mods/tabs", "layouts"] {
            fs::create_dir_all(home.join(p)).unwrap();
        }
        for (p, text) in [
            ("mods/tabs/main.js", "stock tabs"),
            ("layouts/main.json", "{}"),
            ("layouts/personal.json", "{}"),
            ("keys.json", r#"{"global":{"ctrl-p":"custom.action"}}"#),
            ("settings.json", r#"{"zoom":1.2}"#),
        ] {
            fs::write(home.join(p), text).unwrap();
        }
        Self {
            _dir: dir,
            engine: Engine {
                home,
                host: FakeHost {
                    layout: RefCell::new("personal".into()),
                    ..Default::default()
                },
            },
        }
    }
    fn path(&self, p: &str) -> PathBuf {
        self.engine.home.join(p)
    }
    fn unchanged(&self) {
        assert_eq!(
            fs::read_to_string(self.path("mods/tabs/main.js")).unwrap(),
            "stock tabs"
        );
        assert_eq!(
            fs::read_to_string(self.path("settings.json")).unwrap(),
            r#"{"zoom":1.2}"#
        );
        assert_eq!(
            fs::read_to_string(self.path("layouts/main.json")).unwrap(),
            "{}"
        );
    }
    fn receipt(&self) -> Value {
        serde_json::from_slice(
            &fs::read(self.path("data/herdr-auto-titles/installation.json")).unwrap(),
        )
        .unwrap()
    }
    fn save_receipt(&self, value: Value) {
        fs::write(
            self.path("data/herdr-auto-titles/installation.json"),
            value.to_string(),
        )
        .unwrap();
    }
}
#[test]
fn lifecycle_preserves_stock_and_restores_layout() {
    let f = Fixture::new();
    let e = &f.engine;
    assert_eq!(
        e.call("herdr_auto_titles_install", &json!({})).unwrap()["updated"],
        false
    );
    assert_eq!(*e.host.layout.borrow(), "auto-titles-main");
    assert_eq!(e.host.checks.get(), 1);
    f.unchanged();
    assert_eq!(e.install(&json!({})).unwrap()["updated"], true);
    e.enable("terminals").unwrap();
    assert_eq!(*e.host.layout.borrow(), "auto-titles-terminals");
    e.disable().unwrap();
    assert_eq!(*e.host.layout.borrow(), "personal");
    e.enable("win98").unwrap();
    e.uninstall().unwrap();
    assert_eq!(*e.host.layout.borrow(), "personal");
    assert_eq!(e.status().unwrap()["installed"], false);
    assert!(!f.path("mods/auto-titles-tabs").exists());
    f.unchanged();
}
#[test]
fn install_refuses_collision_before_any_writes() {
    let f = Fixture::new();
    fs::create_dir_all(f.path("mods/auto-titles-tabs")).unwrap();
    fs::write(f.path("mods/auto-titles-tabs/main.js"), "mine").unwrap();
    assert!(
        f.engine
            .install(&json!({}))
            .unwrap_err()
            .to_string()
            .contains("not owned")
    );
    assert_eq!(f.engine.host.checks.get(), 0);
    assert!(!f.path("mods/auto-titles-agents").exists());
    f.unchanged();
}
#[test]
fn edited_files_block_update_and_remove() {
    let f = Fixture::new();
    f.engine.install(&json!({"activate":false})).unwrap();
    let path = f.path("mods/auto-titles-tabs/main.js");
    fs::write(&path, "my changes").unwrap();
    for result in [f.engine.install(&json!({})), f.engine.uninstall()] {
        assert!(result.unwrap_err().to_string().contains("Locally edited"));
    }
    assert_eq!(fs::read_to_string(path).unwrap(), "my changes");
    f.unchanged();
}
#[test]
fn validation_failure_does_not_install_payload() {
    let f = Fixture::new();
    f.engine.host.fail_check.set(true);
    assert!(f.engine.install(&json!({})).is_err());
    assert!(!f.path("mods/auto-titles-tabs").exists());
    assert_eq!(f.engine.status().unwrap()["installed"], false);
    f.unchanged();
}
#[test]
fn rechecks_edits_made_while_native_validation_runs() {
    let f = Fixture::new();
    f.engine.install(&json!({"activate":false})).unwrap();
    let p = f.path("mods/auto-titles-tabs/main.js");
    *f.engine.host.on_check.borrow_mut() =
        Some(Box::new(move || fs::write(&p, "concurrent edit").unwrap()));
    assert!(
        f.engine
            .install(&json!({}))
            .unwrap_err()
            .to_string()
            .contains("Locally edited")
    );
    assert_eq!(
        fs::read_to_string(f.path("mods/auto-titles-tabs/main.js")).unwrap(),
        "concurrent edit"
    );
}
#[test]
fn offline_install_reports_pending_and_uninstall_works() {
    let f = Fixture::new();
    f.engine.host.offline.set(true);
    let v = f.engine.install(&json!({})).unwrap();
    assert_eq!(v["activation"]["active"], false);
    assert!(
        v["activation"]["error"]
            .as_str()
            .unwrap()
            .contains("window closed")
    );
    f.engine.uninstall().unwrap();
    f.unchanged();
}
#[test]
fn added_user_files_survive_uninstall() {
    let f = Fixture::new();
    f.engine.install(&json!({"activate":false})).unwrap();
    let p = f.path("mods/auto-titles-tabs/my-notes.txt");
    fs::write(&p, "keep").unwrap();
    f.engine.uninstall().unwrap();
    assert_eq!(fs::read_to_string(p).unwrap(), "keep");
}
#[test]
fn receipt_cannot_remove_other_paths() {
    let f = Fixture::new();
    f.engine.install(&json!({"activate":false})).unwrap();
    let mut r = f.receipt();
    r["files"]["settings.json"] = json!("fake");
    f.save_receipt(r);
    assert!(
        f.engine
            .uninstall()
            .unwrap_err()
            .to_string()
            .contains("Unowned receipt path")
    );
    f.unchanged();
}
#[test]
fn traversal_and_symlink_destinations_are_rejected() {
    let f = Fixture::new();
    for p in [
        "../outside",
        "mods//file",
        "mods/./file",
        "C:/outside",
        "mods\\outside",
    ] {
        assert!(safe_path(&f.engine.home, p).is_err(), "{p}");
    }
    let outside = tempfile::tempdir().unwrap();
    let link = f.path("mods/auto-titles-tabs");
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path(), &link).unwrap();
    #[cfg(windows)]
    if let Err(e) = std::os::windows::fs::symlink_dir(outside.path(), &link) {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            return;
        }
        panic!("{e}");
    }
    assert!(
        f.engine
            .install(&json!({}))
            .unwrap_err()
            .to_string()
            .contains("symlink")
    );
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
}
#[test]
fn scoped_shortcuts_preserve_custom_choices() {
    let f = Fixture::new();
    let p = f.path("keys.json");
    fs::write(&p,json!({"global":{"cmd-p":"mod.open:picker"},"layout:auto-titles-win98":{"cmd-p":"my.action"}}).to_string()).unwrap();
    f.engine.install(&json!({"activate":false})).unwrap();
    let mut keys: Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
    assert_eq!(keys["global"]["cmd-p"], "mod.open:picker");
    assert_eq!(
        keys["layout:auto-titles-main"]["cmd-p"],
        "mod.open:auto-titles-picker"
    );
    assert_eq!(keys["layout:auto-titles-win98"]["cmd-p"], "my.action");
    keys["layout:auto-titles-main"]["cmd-p"] = json!("my.new.action");
    fs::write(&p, keys.to_string()).unwrap();
    f.engine.uninstall().unwrap();
    let keys: Value = serde_json::from_slice(&fs::read(p).unwrap()).unwrap();
    assert_eq!(keys["layout:auto-titles-main"]["cmd-p"], "my.new.action");
    assert!(keys.get("layout:auto-titles-terminals").is_none());
    assert_eq!(keys["layout:auto-titles-win98"]["cmd-p"], "my.action");
}
#[test]
fn win98_runtime_scaling_is_allowed_but_color_edits_are_not() {
    let f = Fixture::new();
    f.engine.install(&json!({"activate":false})).unwrap();
    let p = f.path("layouts/auto-titles-win98.json");
    let baseline = fs::read_to_string(&p).unwrap();
    let mut v: Value = serde_json::from_str(&baseline).unwrap();
    v["font_size"] = json!(16);
    v["children"][0]["children"][0]["size"]["px"] = json!(84);
    v["children"][1]["size"]["px"] = json!(28);
    let pane = &mut v["children"][0]["children"][1]["content"]["pane"];
    pane["header"]["height"] = json!(18);
    pane["border"]["width"] = json!(3);
    pane["focus"]["width"] = json!(3);
    // The native host rewrites JSON numbers as integers and reorders keys.
    let text = v.to_string().replace("10.0", "10");
    assert!(is_auto_fit(text.as_bytes(), &baseline));
    fs::write(&p, text).unwrap();
    f.engine.install(&json!({"activate":false})).unwrap();
    v["background"] = json!("#123456");
    fs::write(&p, v.to_string()).unwrap();
    assert!(
        f.engine
            .uninstall()
            .unwrap_err()
            .to_string()
            .contains("Locally edited")
    );
}
#[test]
fn upgrades_node_v1_receipts() {
    let f = Fixture::new();
    f.engine.install(&json!({"activate":false})).unwrap();
    let mut r = f.receipt();
    r["version"] = json!("1.0.0");
    f.save_receipt(r);
    assert_eq!(
        f.engine.install(&json!({"activate":false})).unwrap()["updated"],
        true
    );
    assert_eq!(f.receipt()["version"], VERSION);
    f.engine.uninstall().unwrap();
    f.unchanged();
}
#[test]
fn all_workflows_share_validated_manifest() {
    let tools = tools();
    for cli in [
        "install",
        "uninstall",
        "status",
        "enable",
        "disable",
        "list",
        "rename",
    ] {
        let t = tools.iter().find(|t| t["cli"] == cli).unwrap();
        assert!(t["http"]["path"].is_string());
        assert!(!t["uiActions"].as_array().unwrap().is_empty());
    }
    let t = tools.iter().find(|t| t["cli"] == "enable").unwrap();
    assert!(validate(t, &json!({"layout":"../other"})).is_err());
    assert!(validate(t, &json!({"layout":"main","extra":true})).is_err());
    assert!(validate(t, &json!({"layout":"main"})).is_ok());
}
#[test]
fn remote_ids_and_names_are_literal_and_validation_precedes_commands() {
    let f = Fixture::new();
    let args = json!({"id":"remote/tb1","name":"$(touch /never); \"hello\""});
    f.engine.call("herdr_tab_title_set", &args).unwrap();
    assert_eq!(
        f.engine.host.calls.borrow().last().unwrap(),
        &vec!["do".to_string(), "tab.rename".into(), args.to_string()]
    );
    let count = f.engine.host.calls.borrow().len();
    for args in [
        json!({"id":"x"}),
        json!({"id":"","name":""}),
        json!({"id":"x","name":7}),
        json!(null),
    ] {
        assert!(f.engine.call("herdr_tab_title_set", &args).is_err());
    }
    assert_eq!(f.engine.host.calls.borrow().len(), count);
}
#[test]
fn snapshots_exclude_unrelated_state_and_reject_stale_layouts() {
    let f = Fixture::new();
    *f.engine.host.layout.borrow_mut() = "auto-titles-main".into();
    let snapshot = json!({"layout":"auto-titles-main","updated_at":"2026-01-01","data":{"spaces":[{"id":"s1","tabs":[{"id":"remote/tb1","title":"Fix search","mode":"automatic"}]}]}});
    fs::write(
        f.path("shared.json"),
        json!({"auto-titles.snapshot":snapshot,"unrelated":"private"}).to_string(),
    )
    .unwrap();
    let result = f.engine.titles().unwrap();
    assert!(!result.to_string().contains("private"));
    *f.engine.host.layout.borrow_mut() = "main".into();
    assert!(
        f.engine
            .titles()
            .unwrap_err()
            .to_string()
            .contains("not active")
    );
    *f.engine.host.layout.borrow_mut() = "auto-titles-main".into();
    let mut snapshot = snapshot;
    snapshot["data"]["spaces"] = json!([]);
    fs::write(
        f.path("shared.json"),
        json!({"auto-titles.snapshot":snapshot}).to_string(),
    )
    .unwrap();
    assert!(
        f.engine
            .titles()
            .unwrap_err()
            .to_string()
            .contains("catching up")
    );
}
#[test]
fn mcp_discovery_structured_output_notifications_and_errors() {
    let request = |m| json!({"jsonrpc":"2.0","id":1,"method":m});
    let call = |_: &str, _: &Value| Ok(json!({"ok":true}));
    assert_eq!(
        transport::rpc(&request("initialize"), call).unwrap()["result"]["protocolVersion"],
        "2025-11-25"
    );
    assert_eq!(
        transport::rpc(&request("tools/list"), call).unwrap()["result"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        7
    );
    assert_eq!(
        transport::rpc(
            &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"test"}}),
            call
        )
        .unwrap()["result"]["structuredContent"]["ok"],
        true
    );
    assert!(
        transport::rpc(
            &json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            call
        )
        .is_none()
    );
    assert_eq!(
        transport::rpc(&request("missing"), call).unwrap()["error"]["code"],
        -32601
    );
    let v = transport::rpc(&request("tools/call"), |_, _| bail!("bad call")).unwrap();
    assert_eq!(v["result"]["isError"], true);
}
#[test]
fn stdio_rejects_bad_json_and_continues() {
    let f = Fixture::new();
    let input = b"bad json\n{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/list\"}\n";
    let mut out = vec![];
    transport::stdio(&f.engine, &input[..], &mut out).unwrap();
    let lines: Vec<Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(lines[0]["error"]["code"], -32700);
    assert_eq!(lines[1]["id"], 7);
}
#[test]
fn embedded_payload_is_complete_and_namespaced() {
    assert!(PAYLOAD.len() > 300);
    for (path, _) in PAYLOAD {
        assert!(path.starts_with("mods/auto-titles-") || path.starts_with("layouts/auto-titles-"));
    }
    for name in ["main", "terminals", "win98"] {
        assert!(
            PAYLOAD
                .iter()
                .any(|(p, _)| *p == format!("layouts/auto-titles-{name}.json"))
        );
    }
}
