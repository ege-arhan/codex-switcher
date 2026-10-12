import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { localizeProductMessage } from "./i18nMessages";
import { shellTranslations } from "./i18nShell";
import { componentTranslations } from "./i18nComponents";
import { claudeTranslations } from "./i18nClaude";
import { getLanguage, LANGUAGE_CHANGED_EVENT, LANGUAGE_STORAGE_KEY, normalizeLanguage, rememberLanguage, translate, type Language } from "./i18nLanguage";
const isTauriRuntime = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
export { getLanguage, getLocale, LANGUAGE_STORAGE_KEY, type Language } from "./i18nLanguage";
const dictionary = { ...shellTranslations, ...componentTranslations, ...claudeTranslations };
export function t(source: string, params?: Record<string, string | number>): string {
  return translate(source, getLanguage(), dictionary, params);
}
export const localizeMessage = (message: string) => localizeProductMessage(message, t);
const LanguageContext = createContext<Language>(getLanguage());
export function setLanguage(value: Language) {
  const language = normalizeLanguage(value);
  rememberLanguage(language);
  try { localStorage.setItem(LANGUAGE_STORAGE_KEY, language); } catch { /* Session selection still works without storage. */ }
  document.documentElement.lang = language;
  window.dispatchEvent(new CustomEvent(LANGUAGE_CHANGED_EVENT, { detail: language }));
  if (isTauriRuntime()) {
    void import("@tauri-apps/api/event").then(({ emit }) => emit(LANGUAGE_CHANGED_EVENT, language)).catch(console.error);
    void import("@tauri-apps/api/core").then(({ invoke }) => invoke("set_ui_language", { language })).catch(console.error);
  }
}
export function I18nProvider({ children }: { children: ReactNode }) {
  const [language, updateLanguage] = useState(getLanguage);
  useEffect(() => {
    const apply = (value: unknown) => {
      const next = normalizeLanguage(value);
      rememberLanguage(next);
      updateLanguage(next);
      document.documentElement.lang = next;
    };
    apply(getLanguage());
    const onChange = (event: Event) => apply((event as CustomEvent).detail);
    const onStorage = (event: StorageEvent) => { if (event.key === LANGUAGE_STORAGE_KEY) apply(event.newValue); };
    window.addEventListener(LANGUAGE_CHANGED_EVENT, onChange);
    window.addEventListener("storage", onStorage);
    let disposed = false;
    let unlisten: (() => void) | undefined;
    if (isTauriRuntime()) {
      void import("@tauri-apps/api/event").then(async ({ listen }) => {
        const stop = await listen<Language>(LANGUAGE_CHANGED_EVENT, ({ payload }) => {
          try { localStorage.setItem(LANGUAGE_STORAGE_KEY, normalizeLanguage(payload)); } catch { /* Session only. */ }
          apply(payload);
        });
        if (disposed) stop(); else unlisten = stop;
      }).catch(console.error);
      void import("@tauri-apps/api/core").then(async ({ invoke }) => {
        let hasStoredLanguage = false;
        try { hasStoredLanguage = localStorage.getItem(LANGUAGE_STORAGE_KEY) !== null; } catch { /* Use the native preference. */ }
        if (hasStoredLanguage) await invoke("set_ui_language", { language: getLanguage() });
        else {
          const stored = await invoke<Language>("get_ui_language");
          if (!disposed) setLanguage(stored);
        }
      }).catch(console.error);
    }
    return () => { disposed = true; unlisten?.(); window.removeEventListener(LANGUAGE_CHANGED_EVENT, onChange); window.removeEventListener("storage", onStorage); };
  }, []);
  return <LanguageContext.Provider value={language}>{children}</LanguageContext.Provider>;
}
export function useI18n() {
  const language = useContext(LanguageContext);
  return { language, locale: language === "tr" ? "tr-TR" as const : "en-US" as const, setLanguage, t };
}
