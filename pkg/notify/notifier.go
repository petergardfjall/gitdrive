package notify

type Event string

func (e Event) String() string {
	return string(e)
}

type Notifier interface {
	// Notify sends a notification for a particular `event` (the `event` is
	// to be considered an event code, used to identify the particular type
	// of notification, for example to facilitate event
	// deduplification). The formatted string is what will be presented/sent
	// to the user.
	Notify(event Event, fmt string, v ...interface{}) error
}
