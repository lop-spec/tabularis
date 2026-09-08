// Build-only Windows compatibility overlay. Never modifies replication/cutover code.
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';

export const GH_OST_COMMIT = '2ed192cd39d8a5173b6ef7c4beaa746a2d3bf1aa';

export function windowsOverlay(source) {
  const logPath = join(source, 'vendor/github.com/openark/golib/log/log.go');
  const mainPath = join(source, 'go/cmd/gh-ost/main.go');
  const log = readFileSync(logPath, 'utf8');
  const main = readFileSync(mainPath, 'utf8');
  if (!log.includes('"log/syslog"') || !main.includes('term.ReadPassword(syscall.Stdin)')) {
    throw new Error('Pinned gh-ost platform layout changed; review compatibility rather than applying a fuzzy patch');
  }
  const overlayDir = join(source, '.tabularis-platform');
  mkdirSync(overlayDir, { recursive: true });
  const overlayLog = join(overlayDir, 'log.go');
  const overlayMain = join(overlayDir, 'main.go');
  const shim = join(source, 'go/platformsyslog');
  mkdirSync(shim, { recursive: true });
  writeFileSync(join(shim, 'syslog_windows.go'), `//go:build windows

// Package platformsyslog explicitly rejects optional Unix syslog on Windows.
// Existing gh-ost stderr logging is unchanged and captured by Tabularis.
package platformsyslog
import ("fmt"; "os")
type Writer struct{}
const LOG_ERR = 3
func New(_ int, _ string) (*Writer, error) { err := fmt.Errorf("Unix syslog is unavailable on Windows; stderr logging remains active"); fmt.Fprintln(os.Stderr, err); return nil, err }
func (w *Writer) Emerg(s string) error { return output(s) }
func (w *Writer) Crit(s string) error { return output(s) }
func (w *Writer) Err(s string) error { return output(s) }
func (w *Writer) Warning(s string) error { return output(s) }
func (w *Writer) Notice(s string) error { return output(s) }
func (w *Writer) Info(s string) error { return output(s) }
func (w *Writer) Debug(s string) error { return output(s) }
func output(s string) error { _, err := fmt.Fprintln(os.Stderr, s); return err }
`);
  writeFileSync(overlayLog, log.replace('"log/syslog"', 'syslog "github.com/github/gh-ost/go/platformsyslog"'));
  writeFileSync(overlayMain, main.replace('term.ReadPassword(syscall.Stdin)', 'term.ReadPassword(int(os.Stdin.Fd()))'));
  const overlay = join(overlayDir, 'overlay.json');
  writeFileSync(overlay, JSON.stringify({ Replace: { [logPath]: overlayLog, [mainPath]: overlayMain } }));
  return overlay;
}

async function run(command, args, cwd) {
  const child = spawn(command, args, { cwd, windowsHide: true, stdio: 'inherit', env: { ...process.env, CGO_ENABLED: '0', GOTOOLCHAIN: 'local' } });
  const [code] = await once(child, 'exit');
  if (code !== 0) throw new Error(`${command} exited with ${code}`);
}

async function main() {
  const source = resolve(process.argv[2] || '.gh-ost-source');
  const testOnly = process.argv.includes('--check-only');
  const overlay = process.platform === 'win32' ? windowsOverlay(source) : null;
  const go = process.env.TABULARIS_GO || 'go';
  const common = ['-mod=vendor', ...(overlay ? [`-overlay=${overlay}`] : [])];
  if (process.platform === 'win32') {
    copyFileSync(resolve('tests/gh-ost/platform_windows_test.go'), join(source, 'go/cmd/gh-ost/tabularis_platform_windows_test.go'));
    await run(go, ['test', ...common, './go/cmd/gh-ost', '-run', '^TestTabularis', '-v'], source);
  }
  if (testOnly) return;
  if (!process.env.CI) throw new Error('Release executable generation is only allowed in CI');
  const root = resolve('src-tauri/target/gh-ost');
  mkdirSync(root, { recursive: true });
  const binary = join(root, process.platform === 'win32' ? 'gh-ost.exe' : 'gh-ost');
  await run(go, ['build', ...common, '-trimpath', '-ldflags=-s -w -X main.AppVersion=1.1.11', '-o', binary, './go/cmd/gh-ost'], source);
  await run(binary, ['--version'], source);
  await run(binary, ['--check-flag', '--panic-on-warnings', '--throttle-control-replicas=replica.example.invalid:3306', '--throttle-http=http://127.0.0.1:1/gate'], source);
  const license = join(root, 'LICENSE');
  copyFileSync(join(source, 'LICENSE'), license);
  if (!process.env.GITHUB_ENV || !existsSync(binary)) throw new Error('Missing CI output environment');
  writeFileSync(process.env.GITHUB_ENV, `TABULARIS_GHOST_BINARY=${binary}\nTABULARIS_GHOST_LICENSE=${license}\n`, { flag: 'a' });
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch(error => { console.error(error); process.exitCode = 1; });
}
