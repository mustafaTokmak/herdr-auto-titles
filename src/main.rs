use anyhow::{Result, bail};
use herdr_auto_titles::{Engine, VERSION, app_home, host::SystemHost, tools, transport};
use serde_json::{Value, json};
fn main() {
    if let Err(error) = run() {
        eprintln!("{}", json!({"error":error.to_string()}));
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version" | "-V") => {
            println!("herdr-auto-titles {VERSION}");
            return Ok(());
        }
        Some("--help" | "-h") => {
            println!(
                "herdr-auto-titles [install [--no-activate]|uninstall|status|enable main|terminals|win98|disable|list|rename ID NAME|--http PORT|--tools|--skill]\nNo arguments: MCP stdio server. Empty NAME restores automatic naming.\nHERDR_APP_HOME selects the target home; HERDR_APP_BIN selects the stock app executable."
            );
            return Ok(());
        }
        Some("--tools") => {
            println!("{}", json!({"tools":tools()}));
            return Ok(());
        }
        Some("--licenses") => {
            print!(
                "{}\n{}",
                include_str!("../LICENSE"),
                include_str!("../THIRD-PARTY-NOTICES")
            );
            return Ok(());
        }
        Some("--skill") => {
            print!("{}", include_str!("../SKILL.md"));
            return Ok(());
        }
        _ => (),
    }
    let home = app_home()?;
    let host = SystemHost::new(home.clone());
    let engine = Engine { home, host };
    if args.is_empty() {
        return transport::stdio(&engine, std::io::stdin().lock(), std::io::stdout().lock());
    }
    if args[0] == "--http" {
        if args.len() > 2 {
            bail!("usage: herdr-auto-titles --http PORT");
        }
        let port = match args.get(1) {
            Some(p) => p.parse::<u16>()?,
            None => 0,
        };
        return transport::http(&engine, port);
    }
    let tool = tools()
        .into_iter()
        .find(|t| t["cli"] == args[0])
        .ok_or_else(|| anyhow::anyhow!("Unknown command; use --help"))?;
    let params: Value = match (args[0].as_str(), &args[1..]) {
        ("rename", [id, name]) => json!({"id":id,"name":name}),
        ("enable", [layout]) => json!({"layout":layout}),
        ("install", [option]) if option == "--no-activate" => json!({"activate":false}),
        ("install" | "uninstall" | "status" | "disable" | "list", []) => json!({}),
        _ => bail!("Invalid arguments; use --help"),
    };
    println!("{}", engine.call(tool["name"].as_str().unwrap(), &params)?);
    Ok(())
}
