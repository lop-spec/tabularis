import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import {
  Copy,
  Eraser,
  RotateCw,
  SquareTerminal,
  StopCircle,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import type { NativeCliKind } from "../../utils/nativeCli";

interface NativeCliStartResponse {
  sessionId: string;
  instanceId: string;
  running: boolean;
  reused: boolean;
  processId?: number;
  executable: string;
  outputBase64: string;
  outputSequence: number;
  outputTruncated: boolean;
}

interface NativeCliOutputEvent {
  sessionId: string;
  instanceId: string;
  sequence: number;
  dataBase64: string;
}

interface NativeCliExitEvent {
  sessionId: string;
  instanceId: string;
  exitCode?: number;
  signal?: string;
  error?: string;
}

interface NativeCliTerminalProps {
  connectionId: string;
  sessionId: string;
  kind: NativeCliKind;
}

type TerminalStatus = "starting" | "running" | "exited" | "error";

function decodeBase64(value: string): Uint8Array {
  if (!value) return new Uint8Array();
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function terminalBufferText(terminal: Terminal): string {
  const buffer = terminal.buffer.active;
  const lines: string[] = [];
  for (let index = 0; index < buffer.length; index += 1) {
    lines.push(buffer.getLine(index)?.translateToString(true) ?? "");
  }
  return lines.join("\n").replace(/\s+$/, "");
}

export function NativeCliTerminal({
  connectionId,
  sessionId,
  kind,
}: NativeCliTerminalProps) {
  const { t } = useTranslation();
  const hostRef = useRef<HTMLDivElement | null>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const instanceIdRef = useRef<string | null>(null);
  const [restartKey, setRestartKey] = useState(0);
  const [status, setStatus] = useState<TerminalStatus>("starting");
  const [runtimePath, setRuntimePath] = useState("");
  const [statusDetail, setStatusDetail] = useState("");

  const copyOutput = useCallback(async () => {
    const terminal = terminalRef.current;
    if (!terminal) return;
    const text = terminal.hasSelection()
      ? terminal.getSelection()
      : terminalBufferText(terminal);
    if (text) await writeText(text);
  }, []);

  const interrupt = useCallback(async () => {
    try {
      await invoke("interrupt_native_cli_session", { sessionId });
    } catch (error) {
      setStatusDetail(errorMessage(error));
    }
  }, [sessionId]);

  const clear = useCallback(async () => {
    terminalRef.current?.clear();
    try {
      await invoke("clear_native_cli_output", { sessionId });
    } catch (error) {
      setStatusDetail(errorMessage(error));
    }
  }, [sessionId]);

  const restart = useCallback(async () => {
    setStatus("starting");
    setStatusDetail("");
    try {
      await invoke("close_native_cli_session", { sessionId });
    } catch {
      // A session that already exited is also safe to restart.
    }
    setRestartKey((value) => value + 1);
  }, [sessionId]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    let disposed = false;
    let ready = false;
    let lastSequence = 0;
    let outputUnlisten: (() => void) | undefined;
    let exitUnlisten: (() => void) | undefined;
    const pendingOutput: NativeCliOutputEvent[] = [];
    let resizeFrame = 0;

    const terminal = new Terminal({
      allowProposedApi: false,
      convertEol: false,
      cursorBlink: true,
      cursorStyle: "bar",
      fontFamily:
        '"JetBrains Mono", "Cascadia Mono", "SFMono-Regular", Consolas, monospace',
      fontSize: 13,
      lineHeight: 1.2,
      scrollback: 20_000,
      theme: {
        background: "#0f1117",
        foreground: "#d7dce2",
        cursor: "#8bb4ff",
        selectionBackground: "#31568a99",
        black: "#171b22",
        brightBlack: "#626b7a",
        red: "#ff6b73",
        brightRed: "#ff8b92",
        green: "#55d187",
        brightGreen: "#75e3a1",
        yellow: "#e5c07b",
        brightYellow: "#f0d493",
        blue: "#6ea8fe",
        brightBlue: "#91bdff",
        magenta: "#c792ea",
        brightMagenta: "#d8a8f2",
        cyan: "#56c8d8",
        brightCyan: "#7ad9e5",
        white: "#d7dce2",
        brightWhite: "#f4f7fb",
      },
    });
    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.open(host);
    terminalRef.current = terminal;

    const fit = () => {
      if (disposed) return;
      cancelAnimationFrame(resizeFrame);
      resizeFrame = requestAnimationFrame(() => {
        if (disposed) return;
        try {
          fitAddon.fit();
        } catch {
          // Hidden panes can temporarily have no measurable geometry.
        }
      });
    };

    const resizeObserver = new ResizeObserver(fit);
    resizeObserver.observe(host);
    const resizeDisposable = terminal.onResize(({ cols, rows }) => {
      if (!ready) return;
      void invoke("resize_native_cli_session", { sessionId, cols, rows }).catch(
        (error) => setStatusDetail(errorMessage(error)),
      );
    });
    const dataDisposable = terminal.onData((data) => {
      if (!ready) return;
      void invoke("write_native_cli_session", { sessionId, data }).catch(
        (error) => setStatusDetail(errorMessage(error)),
      );
    });
    terminal.attachCustomKeyEventHandler((event) => {
      if (event.type !== "keydown" || !event.ctrlKey || !event.shiftKey) {
        return true;
      }
      if (event.key.toLowerCase() === "c") {
        void copyOutput();
        return false;
      }
      if (event.key.toLowerCase() === "v") {
        void readText()
          .then((text) => {
            if (text) {
              return invoke("write_native_cli_session", { sessionId, data: text });
            }
          })
          .catch((error) => setStatusDetail(errorMessage(error)));
        return false;
      }
      return true;
    });

    const writeOutput = (event: NativeCliOutputEvent) => {
      if (
        event.sessionId !== sessionId ||
        event.instanceId !== instanceIdRef.current ||
        event.sequence <= lastSequence
      ) {
        return;
      }
      lastSequence = event.sequence;
      terminal.write(decodeBase64(event.dataBase64));
    };

    const start = async () => {
      try {
        [outputUnlisten, exitUnlisten] = await Promise.all([
          listen<NativeCliOutputEvent>("native-cli-output", (event) => {
            if (!ready) pendingOutput.push(event.payload);
            else writeOutput(event.payload);
          }),
          listen<NativeCliExitEvent>("native-cli-exit", (event) => {
            if (
              event.payload.sessionId !== sessionId ||
              event.payload.instanceId !== instanceIdRef.current
            ) {
              return;
            }
            ready = false;
            setStatus(event.payload.error ? "error" : "exited");
            setStatusDetail(
              event.payload.error ??
                (event.payload.signal
                  ? `signal ${event.payload.signal}`
                  : `exit ${event.payload.exitCode ?? "unknown"}`),
            );
          }),
        ]);
        if (disposed) {
          outputUnlisten?.();
          exitUnlisten?.();
          return;
        }

        fit();
        const response = await invoke<NativeCliStartResponse>(
          "start_native_cli_session",
          {
            connectionId,
            sessionId,
            cols: Math.max(2, terminal.cols),
            rows: Math.max(2, terminal.rows),
          },
        );
        if (disposed) return;

        instanceIdRef.current = response.instanceId;
        terminal.reset();
        terminal.write(decodeBase64(response.outputBase64));
        lastSequence = response.outputSequence;
        ready = response.running;
        setRuntimePath(response.executable);
        setStatus(response.running ? "running" : "exited");
        setStatusDetail(
          response.outputTruncated
            ? t("editor.nativeCliOutputTruncated", {
                defaultValue: "Earlier terminal output was truncated.",
              })
            : "",
        );
        pendingOutput
          .sort((left, right) => left.sequence - right.sequence)
          .forEach(writeOutput);
        pendingOutput.length = 0;
        terminal.focus();
      } catch (error) {
        if (disposed) return;
        ready = false;
        setStatus("error");
        setStatusDetail(errorMessage(error));
        terminal.writeln(`\r\n\x1b[31m${errorMessage(error)}\x1b[0m`);
      }
    };

    void start();

    return () => {
      disposed = true;
      ready = false;
      cancelAnimationFrame(resizeFrame);
      resizeObserver.disconnect();
      resizeDisposable.dispose();
      dataDisposable.dispose();
      outputUnlisten?.();
      exitUnlisten?.();
      terminal.dispose();
      if (terminalRef.current === terminal) terminalRef.current = null;
    };
  }, [connectionId, sessionId, kind, restartKey, copyOutput, t]);

  const label = kind === "mongosh" ? "mongosh" : "redis-cli";
  const statusColor =
    status === "running"
      ? "bg-emerald-400"
      : status === "starting"
        ? "bg-amber-400 animate-pulse"
        : "bg-red-400";

  return (
    <div className="h-full min-h-0 flex flex-col bg-[#0f1117]">
      <div className="h-9 shrink-0 flex items-center gap-2 px-3 border-b border-white/10 bg-[#151923] text-xs text-slate-300">
        <SquareTerminal size={14} className="text-slate-400" />
        <span className="font-mono font-medium">{label}</span>
        <span className={`w-1.5 h-1.5 rounded-full ${statusColor}`} />
        <span className="text-slate-500 truncate" title={runtimePath || statusDetail}>
          {status === "running"
            ? runtimePath
            : statusDetail ||
              t("editor.nativeCliStarting", { defaultValue: "Starting…" })}
        </span>
        <div className="ml-auto flex items-center gap-0.5">
          <button
            type="button"
            onClick={() => void interrupt()}
            className="p-1.5 rounded text-slate-400 hover:text-white hover:bg-white/10"
            title={t("editor.nativeCliInterrupt", { defaultValue: "Interrupt (Ctrl+C)" })}
          >
            <StopCircle size={14} />
          </button>
          <button
            type="button"
            onClick={() => void clear()}
            className="p-1.5 rounded text-slate-400 hover:text-white hover:bg-white/10"
            title={t("editor.nativeCliClear", { defaultValue: "Clear output" })}
          >
            <Eraser size={14} />
          </button>
          <button
            type="button"
            onClick={() => void copyOutput()}
            className="p-1.5 rounded text-slate-400 hover:text-white hover:bg-white/10"
            title={t("editor.nativeCliCopy", { defaultValue: "Copy selection or output" })}
          >
            <Copy size={14} />
          </button>
          <button
            type="button"
            onClick={() => void restart()}
            className="p-1.5 rounded text-slate-400 hover:text-white hover:bg-white/10"
            title={t("editor.nativeCliRestart", { defaultValue: "Restart CLI" })}
          >
            <RotateCw size={14} />
          </button>
        </div>
      </div>
      <div
        ref={hostRef}
        className="native-cli-terminal min-h-0 flex-1 px-2 py-1 overflow-hidden"
        data-native-cli={kind}
      />
    </div>
  );
}
