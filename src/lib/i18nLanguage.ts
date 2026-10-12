export type Language = "tr" | "en";
export const LANGUAGE_STORAGE_KEY = "codex-switcher-language";
export const LANGUAGE_CHANGED_EVENT = "language-changed";
export function normalizeLanguage(value: unknown): Language {
  return value === "en" ? "en" : "tr";
}
let sessionLanguage: Language | undefined;
export function rememberLanguage(value: Language) { sessionLanguage = value; }
export function getLanguage(): Language {
  if (sessionLanguage) return sessionLanguage;
  try { return normalizeLanguage(localStorage.getItem(LANGUAGE_STORAGE_KEY)); }
  catch { return typeof document === "undefined" ? "tr" : normalizeLanguage(document.documentElement.lang); }
}
export function getLocale(): "tr-TR" | "en-US" {
  return getLanguage() === "tr" ? "tr-TR" : "en-US";
}
export function translate(source: string, language: Language, dictionary: Record<string, string>, params: Record<string, string | number> = {}): string {
  const text = language === "tr" ? dictionary[source] ?? source : source;
  return text.replace(/\{(\w+)\}/g, (match, key: string) => Object.prototype.hasOwnProperty.call(params, key) ? String(params[key]) : match);
}
