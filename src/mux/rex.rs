//! Rex backend. Everything goes through the `rex` CLI. Session-scoped
//! subcommands get `-s $REX_BLOCK` (a subcommand flag, so it follows the
//! subcommand) so they resolve against the session holding the caller's pane
//! rather than whichever session the app is showing; `whoami` takes no scope
//! and reads `$REX_BLOCK` itself.
//!
//! Rex blocks don't inherit the caller's environment (a new terminal gets only
//! the login-shell environment plus Rex's own variables), so the TUI command is
//! built from absolute paths and any forwarded variables ride along through an
//! `env K=V` prefix.

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

/// Flavor of a plain Rex terminal block.
const TERMINAL_FLAVOR: &str = "com.superlogical.terminal.shell";

/// Fraction of the caller's pane, on each side, left uncovered around the
/// floating layer so the Claude pane stays visible as a frame.
const INSET: f64 = 0.05;

/// Below this fraction of the window (in either dimension) the caller's pane is
/// too small to host a usable overlay, so the layer centres on the window.
const MIN_RECT_FRACTION: f64 = 0.4;

/// Exit status `rex wait` uses when the target can't be found — the TUI block
/// already closed before the wait started watching it.
const REX_NOT_FOUND: i32 = 3;

#[derive(Debug, Clone)]
pub struct Rex {
    bin: PathBuf,
    /// The caller's block id (`$REX_BLOCK`). Doubles as the `-s` scope: Rex
    /// resolves a block id to the session that contains it.
    block: String,
}

impl Rex {
    /// The Rex backend for this process, if `$REX_BLOCK` is set and the `rex`
    /// binary is on PATH or in `$REX_BIN_DIR`.
    pub fn from_env() -> Option<Rex> {
        let block = env::var("REX_BLOCK").ok().filter(|b| !b.is_empty())?;
        let bin = super::which("rex").or_else(|| {
            let p = PathBuf::from(env::var_os("REX_BIN_DIR")?).join("rex");
            p.is_file().then_some(p)
        })?;
        Some(Rex { bin, block })
    }

    /// `rex <args…>`, run to completion.
    fn run(&self, args: &[&str]) -> Result<Output> {
        Command::new(&self.bin)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .with_context(|| format!("failed to run `rex {}`", args.join(" ")))
    }

    /// Like [`run`](Self::run), but fails on a non-zero exit and parses stdout
    /// as JSON.
    fn json(&self, args: &[&str]) -> Result<Value> {
        let out = self.run(args)?;
        if !out.status.success() {
            bail!(
                "`rex {}` failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        serde_json::from_slice(&out.stdout)
            .with_context(|| format!("`rex {}` returned invalid JSON", args.join(" ")))
    }

    /// Open the TUI in a floating layer over the caller's pane and block until
    /// it exits. The live pane shows around the layer, so no captured backdrop
    /// is needed.
    pub fn popup(&self, file: &Path) -> Result<()> {
        let whoami = self.json(&["whoami", "--json"])?;
        let window_id = str_field(&whoami, "window_id")?;

        // The layer must target the caller's window explicitly: without
        // window_id Rex picks the session's *active* window, which needn't be
        // the one Claude is in.
        let view = self.json(&["api", "call", "-s", &self.block, "session.view"])?;
        let bounds = layer_bounds(find_block_rect(&view, &self.block));

        let params = json!({
            "window_id": window_id,
            "bounds": bounds,
            "focus": true,
            "layout": { "block": {
                "flavor": TERMINAL_FLAVOR,
                "label": "enumerate",
                "options": {
                    "command": tui_command(file)?,
                    "cwd": env::current_dir()?.to_string_lossy(),
                },
            }},
        });
        let created = self.json(&[
            "api",
            "call",
            "-s",
            &self.block,
            "session.new_layer",
            &params.to_string(),
        ])?;
        let layer_id = str_field(&created, "layer_id")?.to_string();
        let block_id = created["block_ids"][0]
            .as_str()
            .context("session.new_layer returned no block id")?
            .to_string();

        // If this process is killed while waiting (the agent stops the
        // backgrounded call), take the layer down with it instead of leaving
        // an orphaned editor floating over the pane.
        let guard = Arc::new(Mutex::new(Some((self.clone(), layer_id.clone()))));
        {
            let guard = Arc::clone(&guard);
            let _ = ctrlc::set_handler(move || {
                if let Some((rex, layer)) = guard.lock().ok().and_then(|mut g| g.take()) {
                    rex.close_layer(&layer);
                }
                std::process::exit(130);
            });
        }

        let code = self.wait(&block_id)?;
        let pending = guard.lock().ok().and_then(|mut g| g.take());
        if code != 0 {
            // A TUI that fails within Rex's quick-exit window leaves its
            // terminal (and so the layer) open; grab what it printed, then
            // close it ourselves.
            let screen = self
                .run(&["capture", "-s", &self.block, "-b", &block_id, "--trim"])
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            if let Some((rex, layer)) = pending {
                rex.close_layer(&layer);
            }
            bail!("enumerate TUI exited with status {code}:\n{screen}");
        }
        Ok(())
    }

    /// Run the TUI in a new Rex window and block until it exits.
    pub fn window(&self, file: &Path) -> Result<()> {
        let cwd = env::current_dir()?;
        let mut args = vec![
            "run".to_string(),
            "-s".to_string(),
            self.block.clone(),
            "--wait".to_string(),
            "--json".to_string(),
            "--label".to_string(),
            "enumerate".to_string(),
            "-c".to_string(),
            cwd.to_string_lossy().into_owned(),
            "--".to_string(),
        ];
        args.extend(tui_command(file)?);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = self.run(&args)?;
        if !out.status.success() {
            bail!(
                "enumerate TUI exited with status {}: {}",
                out.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(())
    }

    /// Width of the caller's terminal block in columns.
    pub fn pane_width(&self) -> Option<usize> {
        let size = self
            .json(&["block", "call", "-b", &self.block, "size"])
            .ok()?;
        size["columns"].as_u64().map(|c| c as usize)
    }

    /// Block until `block_id`'s program exits and return its exit status. A
    /// block that's already gone counts as a clean exit: the TUI closed before
    /// the wait could start watching.
    fn wait(&self, block_id: &str) -> Result<i32> {
        let out = self.run(&["wait", "-s", &self.block, block_id, "--json"])?;
        if let Ok(v) = serde_json::from_slice::<Value>(&out.stdout)
            && let Some(code) = v["exit_code"].as_i64()
        {
            return Ok(code as i32);
        }
        match out.status.code() {
            Some(REX_NOT_FOUND) | Some(0) => Ok(0),
            Some(code) => bail!(
                "`rex wait` failed with status {code}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
            None => bail!("`rex wait` was killed by a signal"),
        }
    }

    /// Best-effort: close a floating layer, ignoring a layer that's already
    /// gone.
    fn close_layer(&self, layer_id: &str) {
        let params = json!({ "layer_id": layer_id }).to_string();
        let _ = self.run(&[
            "api",
            "call",
            "-s",
            &self.block,
            "session.close_layer",
            &params,
        ]);
    }
}

/// `[env ENUMERATE_DEBUG=… ] <abs exe> <abs file>`.
fn tui_command(file: &Path) -> Result<Vec<String>> {
    let exe = env::current_exe().context("could not determine current binary path")?;
    let file = file
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", file.display()))?;
    let mut cmd = Vec::new();
    // Forward ENUMERATE_DEBUG so debug logging works through --popup/--window.
    if let Some(val) = env::var_os("ENUMERATE_DEBUG") {
        cmd.push("env".to_string());
        cmd.push(format!("ENUMERATE_DEBUG={}", val.to_string_lossy()));
    }
    cmd.push(exe.to_string_lossy().into_owned());
    cmd.push(file.to_string_lossy().into_owned());
    Ok(cmd)
}

fn str_field<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .with_context(|| format!("rex reply is missing {key:?}"))
}

/// The `[x, y, w, h]` window-fraction rect of `block_id` in a `session.view`
/// reply, found by searching every window's layout tree for the node naming
/// that block.
fn find_block_rect(view: &Value, block_id: &str) -> Option<[f64; 4]> {
    fn search(v: &Value, block_id: &str) -> Option<[f64; 4]> {
        match v {
            Value::Object(map) => {
                if map.get("block_id").and_then(Value::as_str) == Some(block_id)
                    && let Some(rect) = map.get("rect").and_then(parse_rect)
                {
                    return Some(rect);
                }
                map.values().find_map(|child| search(child, block_id))
            }
            Value::Array(items) => items.iter().find_map(|child| search(child, block_id)),
            _ => None,
        }
    }
    search(view, block_id)
}

fn parse_rect(v: &Value) -> Option<[f64; 4]> {
    let a = v.as_array()?;
    if a.len() != 4 {
        return None;
    }
    let mut out = [0.0; 4];
    for (slot, item) in out.iter_mut().zip(a) {
        *slot = item.as_f64()?;
    }
    Some(out)
}

/// Bounds for the floating layer: the caller's pane inset by [`INSET`] on each
/// side, or a window-centred rect when the pane is unknown or too small.
fn layer_bounds(rect: Option<[f64; 4]>) -> [f64; 4] {
    let [x, y, w, h] = match rect {
        Some(r) if r[2] >= MIN_RECT_FRACTION && r[3] >= MIN_RECT_FRACTION => r,
        _ => [0.0, 0.0, 1.0, 1.0],
    };
    [
        x + w * INSET,
        y + h * INSET,
        w * (1.0 - 2.0 * INSET),
        h * (1.0 - 2.0 * INSET),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_rect_in_nested_split() {
        let view = json!({"windows": [{"layers": [{"layout": {
            "rect": [0.0, 0.0, 1.0, 1.0],
            "split": {
                "direction": "horizontal",
                "before": {"rect": [0.0, 0.0, 0.5, 1.0], "block_id": "block:a"},
                "after": {"rect": [0.5, 0.0, 0.5, 1.0], "block_id": "block:b"},
            }
        }}]}]});
        assert_eq!(
            find_block_rect(&view, "block:b"),
            Some([0.5, 0.0, 0.5, 1.0])
        );
        assert_eq!(find_block_rect(&view, "block:c"), None);
    }

    #[test]
    fn bounds_inset_within_pane() {
        let b = layer_bounds(Some([0.5, 0.0, 0.5, 1.0]));
        assert!((b[0] - 0.525).abs() < 1e-9);
        assert!((b[2] - 0.45).abs() < 1e-9);
    }

    #[test]
    fn bounds_fall_back_to_window_for_small_panes() {
        assert_eq!(layer_bounds(Some([0.0, 0.0, 0.2, 1.0])), layer_bounds(None));
    }
}
