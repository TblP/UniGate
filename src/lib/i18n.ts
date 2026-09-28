import { useAppStore } from "../store/useAppStore";
import messages from "./translations.json";

export type Language = "ru" | "en";

export function currentLanguage(value: string | undefined): Language {
  return value === "en" ? "en" : "ru";
}

export function translate(language: Language, key: string, values?: Record<string, string | number>): string {
  const catalog = messages[language] as Record<string, string>;
  const fallback = messages.ru as Record<string, string>;
  const message = catalog[key] ?? fallback[key] ?? key;
  return message.replace(/\{(\w+)\}/g, (match, name: string) =>
    values?.[name] === undefined ? match : String(values[name]),
  );
}

/** Keep full native diagnostics in the console while showing a readable message. */
export function displayError(language: Language, error: unknown, fallbackKey: string): string {
  const detail = String(error);
  if (language === "ru" || !/[А-Яа-яЁё]/.test(detail)) return detail;
  console.error(detail);
  return translate(language, fallbackKey);
}

export function useI18n() {
  const language = useAppStore((state) => currentLanguage(state.settings?.language));
  return {
    language,
    t: (key: string, values?: Record<string, string | number>) => translate(language, key, values),
  };
}
