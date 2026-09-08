//go:build windows

package main

import (
    "flag"
    "net"
    "os"
    "os/exec"
    "path/filepath"
    "strings"
    "syscall"
    "testing"

    "github.com/go-ini/ini"
)

func TestTabularisWindowsSocket(t *testing.T) {
    directory := t.TempDir()
    listener, err := net.Listen("unix", filepath.Join(directory, "c.sock"))
    if err != nil { t.Fatal(err) }
    listener.Close()
}

func TestTabularisCredentialEncoding(t *testing.T) {
    password := "p#;=\\word\" $!"
    cfg, err := ini.Load([]byte("[client]\nuser = \"\"\"fixture\"\"\"\npassword = \"\"\"" + password + "\"\"\"\n"))
    if err != nil { t.Fatal(err) }
    if cfg.Section("client").Key("password").String() != password { t.Fatal("Password encoding changed the credential") }
}

func TestTabularisWindowsCLI(t *testing.T) {
    if mode := os.Getenv("TABULARIS_GHOST_TEST_MODE"); mode != "" {
        flag.CommandLine = flag.NewFlagSet("gh-ost", flag.ExitOnError)
        os.Args = []string{"gh-ost"}
        if mode == "version" { os.Args = append(os.Args, "--version") } else {
            os.Args = append(os.Args, "--check-flag", "--panic-on-warnings", "--throttle-http=http://127.0.0.1:1/gate", "--throttle-control-replicas=replica.example.invalid:3306", "--mysql-timeout=5")
        }
        main()
        return
    }
    executable, err := os.Executable()
    if err != nil { t.Fatal(err) }
    for _, mode := range []string{"version", "flags"} {
        command := exec.Command(executable, "-test.run=^TestTabularisWindowsCLI$")
        command.Env = append(os.Environ(), "TABULARIS_GHOST_TEST_MODE="+mode)
        command.SysProcAttr = &syscall.SysProcAttr{HideWindow: true, CreationFlags: 0x08000000}
        output, err := command.CombinedOutput()
        if err != nil { t.Fatalf("CLI %s: %v: %s", mode, err, output) }
        if mode == "version" && !strings.Contains(string(output), "git commit:") { t.Fatalf("Missing version output: %s", output) }
    }
}
