# Per-day alarm state rolls over at the end of Isha, not at 00:00

Fired events, Done marks and Snoozes belong to a **Prayer Day**, which only moves to the next date once yesterday's Isha window has closed. Isha ends at Islamic midnight, and in many places and seasons that falls after 00:00 on the clock (London in summer ≈ 01:05). With a reset at 00:00, the app wiped Isha's Done mark and Snooze, never rang its Before-End Warning, and showed "next: Fajr" while Isha was still valid.

## Considered Options

- **Reset at 00:00 and also check yesterday's events.** Rejected: Done and Snoozes are stored per prayer, not per date, so after 00:00 yesterday's Isha state would be gone (or would clash with today's).
- **Treat Isha as ending at Fajr.** Rejected: that changes the religious ruling the app follows instead of fixing the bug.

## Consequences

Between 00:00 and the end of Isha, the Today screen still shows yesterday's timetable (with Isha marked "now"). The months for yesterday, today and tomorrow are kept loaded so the Prayer Day can be worked out on the 1st of a month.
