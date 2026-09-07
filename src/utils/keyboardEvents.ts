// Backported from upstream c395fc71: composition keys belong to the IME.
export function isTextCompositionKeyEvent(event: KeyboardEvent): boolean {
  return event.isComposing || event.key === "Dead" || event.key === "Process" ||
    event.key === "Unidentified" || event.keyCode === 229;
}
