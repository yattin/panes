const APP_SHORTCUTS_ALLOWED_WHILE_TERMINAL_FOCUSED = new Set(["d", "e", "k", "p", "t"]);

export function shouldHandleAppShortcutWhileTerminalFocused(
  key: string,
  shiftKey: boolean,
): boolean {
  const normalizedKey = key.toLowerCase();

  // Keep browser/WebView save-page suppression active in every focus state.
  if (normalizedKey === "s" && !shiftKey) {
    return true;
  }

  // These shortcuts are owned by the app and do not have a native-menu fallback.
  if (normalizedKey === "i" && shiftKey) {
    return true;
  }

  if (normalizedKey === "n" && shiftKey) {
    return true;
  }

  return APP_SHORTCUTS_ALLOWED_WHILE_TERMINAL_FOCUSED.has(normalizedKey);
}
