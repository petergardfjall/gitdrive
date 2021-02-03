package notify

type NoOp struct{}

func NewNoOp() *NoOp {
	return &NoOp{}
}

func (d *NoOp) Notify(event Event, format string, v ...interface{}) error {
	return nil
}
