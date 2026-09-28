// Форматирование байтов для отображения трафика.

import { currentLanguage, translate, type Language } from "./i18n";
import { useAppStore } from "../store/useAppStore";

const UNIT_KEYS = ["units.byte", "units.kilobyte", "units.megabyte", "units.gigabyte", "units.terabyte"];

function humanBytes(bytes: number, language: Language): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return `0 ${translate(language, UNIT_KEYS[0])}`;
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), UNIT_KEYS.length - 1);
  const value = bytes / 1024 ** i;
  return `${value.toFixed(i === 0 ? 0 : 1)} ${translate(language, UNIT_KEYS[i])}`;
}

/** Скорость: байт/с → "1.2 МБ/с". */
export function formatSpeed(bytesPerSec: number): string {
  const language = currentLanguage(useAppStore.getState().settings?.language);
  return translate(language, "units.perSecond", { value: humanBytes(bytesPerSec, language) });
}

/** Объём: байт → "345 МБ". */
export function formatBytes(bytes: number): string {
  return humanBytes(bytes, currentLanguage(useAppStore.getState().settings?.language));
}
