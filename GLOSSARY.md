# Prayer Times

A desktop app that tells the user when each daily prayer's time begins and ends, and rings alarms so they don't miss it.

## Language

### Time

**Prayer**:
One of the five daily prayers: Fajr, Dhuhr, Asr, Maghrib, Isha. Sunrise and midnight are boundaries, not prayers.
_Avoid_: Salah, namaz, waqt (in code and docs)

**Prayer Window**:
The span during which a prayer may be performed, from its start time until the next boundary: Fajr → sunrise, Dhuhr → Asr, Asr → Maghrib, Maghrib → Isha, Isha → midnight.
_Avoid_: Prayer time (ambiguous: start instant or whole span), slot, period

**Midnight**:
The Islamic midnight given by the timetable (halfway between sunset and Fajr), which ends Isha's window. It can fall after 00:00 on the clock.
_Avoid_: 00:00, end of day

**Prayer Day**:
The day that the user's prayer state belongs to. It is the location's calendar date, except while yesterday's Isha window is still open past 00:00.
_Avoid_: Today (when the calendar date is meant), calendar day

**Timetable**:
The start times, sunrise and midnight for a location on a given date, as calculated by the chosen Calculation Method and Asr School.
_Avoid_: Schedule (that means the alarms), timings

**Location Time**:
Wall-clock time in the timezone of the user's chosen city. All prayer times are expressed in it, never in the device's timezone.
_Avoid_: Local time (ambiguous)

### Location

**Location**:
The place the user's prayer times are calculated for: a city in a country, with its timezone.
_Avoid_: Address, place (in UI copy)

**Picked City**:
A Location chosen from city search results, so its exact coordinates are known and times are calculated for that spot.

**Typed City**:
A Location given only by name, which the times service looks up by itself and may match to the wrong place. Only a fallback when search can't find the town (and how older versions stored every Location).
_Avoid_: Manual city, custom city

### Calculation

**Calculation Method**:
The convention (e.g. Muslim World League) used to calculate Fajr and Isha from the sun's angle.

**Asr School**:
The juristic rule for when Asr starts: Standard (Shafi'i, Maliki, Hanbali) or Hanafi (later).
_Avoid_: Madhab (too broad)

### Alarms

**Event**:
A planned moment in a Prayer Day when the app should alert the user about one prayer: its Get-Ready Alarm, its Main Alarm or its Before-End Warning.
_Avoid_: Notification (that is one way an Alarm is shown)

**Own Prayer Time**:
When the user actually prays a prayer: its start time, or a set number of minutes after it ("I pray 75 min after it starts", e.g. with a mosque), never in the window's last 5 minutes. It follows the start time from day to day. The Get-Ready and Main Alarms are tied to it; the Prayer Window and Before-End Warning are not.
_Avoid_: Jamaat time, iqamah (in code), delay

**Get-Ready Alarm**:
An optional heads-up a set number of minutes (at most an hour) before the Own Prayer Time ("Asr in 10 minutes"), shown as "Get ready" in the UI. It never rings before the Prayer Day's calendar date begins, and it can't mark the prayer Done.
_Avoid_: Early alarm, pre-alarm

**Main Alarm**:
A prayer's primary alert, at its Own Prayer Time (the start of its window unless set later). Shown as "Prayer time" in the UI; it can be switched off.
_Avoid_: Start alarm, reminder (when the main one is meant)

**Before-End Warning**:
A final alert a set number of minutes before a Prayer Window closes ("you will miss it"), shown as "Don't miss it" in the UI.
_Avoid_: Pre-end alarm, last call

**Alarm**:
An Event or Snooze that is ringing: sound, notification and/or popup, until the user marks it done, snoozes it, dismisses it, or it stops on its own.
_Avoid_: Ring, alert (as a noun for the state)

**Snooze**:
A one-off reminder for one prayer, due a chosen number of minutes after the user postponed its Alarm.

**Done**:
The user's mark that they have prayed a prayer in the current Prayer Day. A done prayer gets no more Alarms until the next Prayer Day.
_Avoid_: Completed, prayed, checked

**Missed Event**:
An Event whose moment passed more than 10 minutes ago without ringing (the app was closed or the machine was asleep). It is skipped silently.
