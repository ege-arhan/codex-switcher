export const THEME_CHANGED_EVENT = "theme-changed";
export const THEME_STORAGE_KEY = "codex-switcher-theme";

export const THEMES = [
  { id: "macos", name: "macOS", description: "Quiet and familiar", dark: false },
  { id: "windows-xp", name: "Windows XP", description: "Classic blue and silver", dark: false },
  { id: "windows-7", name: "Windows 7", description: "Soft glass and sky blue", dark: false },
  { id: "terminal", name: "Terminal", description: "Phosphor green on black", dark: true },
  { id: "paper", name: "Paper", description: "Warm paper and ink", dark: false },
  { id: "nord", name: "Nord", description: "Cool and comfortable", dark: true },
  { id: "mono", name: "Mono", description: "Black, white and nothing extra", dark: false },
] as const;
export type ThemeId = (typeof THEMES)[number]["id"];
// Keep old event payloads compatible with tray windows from a previous version.
export type ThemeMode = ThemeId | "light" | "dark";

export function normalizeTheme(value: unknown): ThemeId {
  if (value === "dark") return "nord";
  if (value === "light") return "macos";
  return THEMES.find((theme) => theme.id === value)?.id ?? "macos";
}

export function readStoredTheme(): ThemeId {
  if (typeof window === "undefined") return "macos";
  try {
    return normalizeTheme(window.localStorage.getItem(THEME_STORAGE_KEY));
  } catch {
    return "macos";
  }
}

export function applyTheme(value: ThemeMode): void {
  const theme = normalizeTheme(value);
  document.documentElement.dataset.theme = theme;
  document.documentElement.classList.toggle("dark", THEMES.some((entry) => entry.id === theme && entry.dark));
}

export function syncThemeFromStorage(): void {
  applyTheme(readStoredTheme());
}
