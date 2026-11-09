/**
 * Turns an error from the backend or a plugin into a sentence a user can act
 * on. Messages the app writes itself (already plain: "Dhuhr hasn't started
 * yet", "This is not a Prayer Times backup file") pass through; technical
 * text (OS errors, database or parser detail) becomes a calm explanation.
 */
export function friendlyError(e: unknown): string {
  const raw = (e instanceof Error ? e.message : String(e)).trim();
  if (/network|timed? ?out|dns|connect|error sending request|offline/i.test(raw)) {
    return "Network error — check your internet connection and try again.";
  }
  if (/history database|database is locked|sqlite/i.test(raw)) {
    return "Your prayer history couldn't be read or saved just now. Alarms still work — try again in a moment.";
  }
  if (/permission denied|access is denied|os error 13|os error 5\b/i.test(raw)) {
    return "The app isn't allowed to use that file or folder. Pick another location.";
  }
  if (/no such file|os error 2\b|cannot find the file/i.test(raw)) {
    return "That file couldn't be found.";
  }
  if (!raw || /os error|invalid type|expected .* at line|panicked|serde|unwrap|ipc|invoke/i.test(raw)) {
    return "Something went wrong. Please try again.";
  }
  return raw;
}
