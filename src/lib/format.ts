// Форматирование байтов для отображения трафика.

const UNITS = ["Б", "КБ", "МБ", "ГБ", "ТБ"];

function humanBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 Б";
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), UNITS.length - 1);
  const value = bytes / 1024 ** i;
  return `${value.toFixed(i === 0 ? 0 : 1)} ${UNITS[i]}`;
}

/** Скорость: байт/с → "1.2 МБ/с". */
export function formatSpeed(bytesPerSec: number): string {
  return `${humanBytes(bytesPerSec)}/с`;
}

/** Объём: байт → "345 МБ". */
export function formatBytes(bytes: number): string {
  return humanBytes(bytes);
}
