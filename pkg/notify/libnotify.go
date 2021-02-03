package notify

import (
	"fmt"
	"os"

	"github.com/petergardfjall/gitdrive/pkg/executer"
)

type LibNotify struct{}

func NewLibNotify() *LibNotify {
	return &LibNotify{}
}

func (n *LibNotify) Notify(event Event, format string, v ...interface{}) error {
	dir, err := os.Getwd()
	if err != nil {
		return err
	}
	e := executer.NewShell(dir)
	_, err = e.Execf("notify-send '%s' '%s'", event, fmt.Sprintf(format, v...))
	return err
}
