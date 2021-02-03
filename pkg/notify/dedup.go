package notify

import (
	"time"

	"github.com/rs/zerolog/log"
)

type Dedup struct {
	wrapped Notifier

	dedupInterval time.Duration
	firedEvents   map[Event]time.Time
}

func NewDedup(n Notifier, dedupInterval time.Duration) *Dedup {
	return &Dedup{
		wrapped: n,

		dedupInterval: dedupInterval,
		firedEvents:   make(map[Event]time.Time),
	}
}

func (d *Dedup) Notify(event Event, format string, v ...interface{}) error {
	if lastFired, ok := d.firedEvents[event]; ok {
		dedupUntil := lastFired.Add(d.dedupInterval)
		if time.Now().Before(dedupUntil) {
			log.Debug().Str("event", event.String()).Time("until", dedupUntil).
				Msg("notification dedup")
			return nil
		}
	}

	if err := d.wrapped.Notify(event, format, v...); err != nil {
		return err
	}

	d.firedEvents[event] = time.Now()
	return nil
}
