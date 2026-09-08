import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync("src-tauri/src/lib.rs", "utf8");

describe("main-window close contract", () => {
  it("exits via the normal Tauri lifecycle instead of hiding or suspending", () => {
    const handler = source.slice(source.indexOf('if args.explain.is_none()'), source.indexOf('// If the user launched with'));
    expect(handler).toContain('app.get_webview_window("main")');
    expect(handler).toContain("tauri::WindowEvent::CloseRequested");
    expect(handler).toContain("handle.exit(0)");
    expect(handler).not.toContain(".hide()");
    expect(source).not.toContain("TABULARIS_CLOSE_TO_HIDE");
    expect(source).not.toContain("set_main_webview_suspended");
  });

  it("retains exit backups, tunnel shutdown, and independent CLI explain mode", () => {
    expect(source).toContain("backup::run_exit_backup(app_handle)");
    expect(source).toContain("crate::ssh_tunnel::stop_all_tunnels()");
    expect(source).toContain("if let Some(path) = args.explain.clone()");
    expect(source).toContain("explain_import::spawn_visual_explain_window");
  });
});
