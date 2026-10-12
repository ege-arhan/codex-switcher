import assert from "node:assert/strict";
import test from "node:test";
import { getLanguage, getLocale, LANGUAGE_STORAGE_KEY, normalizeLanguage, translate } from "../src/lib/i18nLanguage.ts";
import { shellTranslations } from "../src/lib/i18nShell.ts";
import { componentTranslations } from "../src/lib/i18nComponents.ts";
import { themeAppearanceTranslations } from "../src/lib/i18nThemeAppearance.ts";
import { claudeTranslations } from "../src/lib/i18nClaude.ts";
import { localizeProductMessage } from "../src/lib/i18nMessages.ts";

const dictionary = { ...shellTranslations, ...componentTranslations, ...claudeTranslations, ...themeAppearanceTranslations };

test("language preference persists and selects matching date and number locales", () => {
  const values = new Map<string, string>();
  const original = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
  Object.defineProperty(globalThis, "localStorage", { configurable: true, value: { getItem: (key: string) => values.get(key) ?? null } });
  try {
    assert.equal(getLanguage(), "tr");
    assert.equal(getLocale(), "tr-TR");
    values.set(LANGUAGE_STORAGE_KEY, "en");
    assert.equal(getLanguage(), "en");
    assert.equal(getLocale(), "en-US");
    assert.notEqual(new Intl.NumberFormat("tr-TR").format(1.5), new Intl.NumberFormat(getLocale()).format(1.5));
    values.set(LANGUAGE_STORAGE_KEY, "invalid");
    assert.equal(getLanguage(), "tr");
    assert.equal(normalizeLanguage(null), "tr");
  } finally {
    if (original) Object.defineProperty(globalThis, "localStorage", original);
    else Reflect.deleteProperty(globalThis, "localStorage");
  }
});

test("translation uses the selected language and interpolates values literally", () => {
  assert.equal(translate("Settings", "tr", dictionary), "Ayarlar");
  assert.equal(translate("Settings", "en", dictionary), "Settings");
  assert.equal(translate("Warm-up sent for {name}", "tr", dictionary, { name: "$& {private}" }), "$& {private} için ısınma isteği gönderildi");
  assert.equal(translate("{count} Codex running", "en", dictionary, { count: 2 }), "2 Codex running");
  assert.equal(translate("Unknown technical diagnostic", "tr", dictionary), "Unknown technical diagnostic");
});

test("every Turkish translation retains exactly the source placeholders", () => {
  const placeholders = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map(m => m[1]).sort();
  for (const [source, translated] of Object.entries(dictionary)) {
    assert.ok(translated.trim(), source);
    assert.deepEqual(placeholders(translated), placeholders(source), source);
  }
});

test("native recovery messages translate without rewriting account or prompt values", () => {
  const tr = (source: string, params?: Record<string, string | number>) => translate(source, "tr", dictionary, params);
  assert.equal(localizeProductMessage("Switched to 'test@example.com' and started continuations in 2 desktop sessions", tr), "'test@example.com' hesabına geçildi ve 2 masaüstü oturumunda devam başlatıldı");
  assert.equal(localizeProductMessage("Switched to account 'A' and resumed session in-place with 'continue'", tr), "'A' hesabına geçildi ve mevcut oturuma 'continue' mesajıyla devam edildi");
  assert.equal(localizeProductMessage("OS error: EACCES /secret/path", tr), "OS error: EACCES /secret/path");
});

test("Claude usage throttling is localized without asking for reauthentication", () => {
  const source = "Claude usage checks are temporarily rate limited (HTTP 429). Automatic retry is pending; signing in again is not required.";
  const tr = (source: string, params?: Record<string, string | number>) => translate(source, "tr", dictionary, params);
  assert.match(localizeProductMessage(source, tr), /yeniden giriş yapman gerekmiyor/);
  assert.equal(localizeProductMessage("Claude usage check failed (HTTP 503)", tr), "Claude kullanımı kontrol edilemedi (HTTP 503)");
});
