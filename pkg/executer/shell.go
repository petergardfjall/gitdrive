package executer

import (
	"bytes"
	"fmt"
	"os/exec"

	"github.com/rs/zerolog/log"
)

type Shell struct {
	workDir string
}

func NewShell(dir string) *Shell {
	return &Shell{workDir: dir}
}

// Execf executes the given (formatted) shell command.
func (s *Shell) Execf(format string, val ...interface{}) (output string, err error) {
	cmd := fmt.Sprintf(format, val...)

	c := exec.Command("/bin/sh", "-c", cmd)
	c.Dir = s.workDir

	log.Debug().Msg(cmd)
	var b bytes.Buffer
	c.Stdout = &b
	c.Stderr = &b
	if err := c.Run(); err != nil {
		return "", fmt.Errorf("failed to exec '%s': %v\n\n%s", cmd, err, b.String())
	}

	return b.String(), nil
}
