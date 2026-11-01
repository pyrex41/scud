//go:build linux || darwin

package main

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestInstallerOnlyInstallsScudAndPreservesExistingOnFailure(t *testing.T) {
	script, err := filepath.Abs(filepath.Join("..", "..", "install.sh"))
	if err != nil {
		t.Fatal(err)
	}
	for _, fail := range []bool{false, true} {
		t.Run(map[bool]string{false: "success", true: "failed-download"}[fail], func(t *testing.T) {
			root := t.TempDir()
			bin, dest := filepath.Join(root, "tools"), filepath.Join(root, "install")
			for _, dir := range []string{bin, dest} {
				if err := os.Mkdir(dir, 0700); err != nil {
					t.Fatal(err)
				}
			}
			write := func(path, text string) {
				t.Helper()
				if err := os.WriteFile(path, []byte(text), 0700); err != nil {
					t.Fatal(err)
				}
			}
			write(filepath.Join(bin, "uname"), "#!/bin/sh\ncase \"$1\" in -s) echo Linux;; -m) echo x86_64;; *) exit 1;; esac\n")
			write(filepath.Join(bin, "curl"), `#!/bin/sh
out=""
url=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) out="$2"; shift 2;;
    http*) url="$1"; shift;;
    *) shift;;
  esac
done
printf '%s\n' "$url" >> "$REQUEST_LOG"
case "$url" in
  https://api.github.com/repos/pyrex41/scud/releases/latest)
    printf '%s\n' '{"tag_name":"v3.0.0"}';;
  https://github.com/pyrex41/scud/releases/download/v3.0.0/scud-linux-amd64)
    if [ "$FAIL_DOWNLOAD" = "1" ]; then printf partial > "$out"; exit 22; fi
    printf '%s\n' '#!/bin/sh' "printf 'scud version v3.0.0\\n'" > "$out";;
  *) echo 'Unexpected network request' >&2; exit 99;;
esac
`)
			original := "#!/bin/sh\necho original\n"
			write(filepath.Join(dest, "scud"), original)
			log := filepath.Join(root, "requests")
			command := exec.CommandContext(t.Context(), "sh", script)
			command.Dir = root
			command.Env = append(os.Environ(), "PATH="+bin+string(os.PathListSeparator)+os.Getenv("PATH"), "INSTALL_DIR="+dest, "REQUEST_LOG="+log, "FAIL_DOWNLOAD="+map[bool]string{false: "0", true: "1"}[fail])
			output, runErr := command.CombinedOutput()
			if (runErr != nil) != fail {
				t.Fatalf("run err=%v output=%s", runErr, output)
			}
			requests, err := os.ReadFile(log)
			if err != nil {
				t.Fatal(err)
			}
			if strings.Count(strings.TrimSpace(string(requests)), "\n") != 1 || strings.Contains(string(requests), "/rho") {
				t.Fatalf("extra/unexpected downloads: %s", requests)
			}
			contents, err := os.ReadFile(filepath.Join(dest, "scud"))
			if err != nil {
				t.Fatal(err)
			}
			if fail && string(contents) != original {
				t.Fatal("failed download changed the installed binary")
			}
			if !fail && !strings.Contains(string(contents), "v3.0.0") {
				t.Fatalf("wrong installed content: %s", contents)
			}
			if temporary, _ := filepath.Glob(filepath.Join(dest, ".scud-download.*")); len(temporary) != 0 {
				t.Fatalf("leaked downloads: %v", temporary)
			}
		})
	}
}
