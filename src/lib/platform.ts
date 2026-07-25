/** Грубое определение ОС по user-agent webview — без доп. зависимостей. */
export const isMacOS =
  typeof navigator !== "undefined" &&
  /Mac|Macintosh|Mac OS X/i.test(
    navigator.userAgent || (navigator as { platform?: string }).platform || ""
  );

export const isWindows =
  typeof navigator !== "undefined" &&
  /Windows|Win32|Win64/i.test(
    navigator.userAgent || (navigator as { platform?: string }).platform || ""
  );

const androidPreview =
  import.meta.env.DEV &&
  typeof window !== "undefined" &&
  new URLSearchParams(window.location.search).get("platform") === "android";

export const isAndroid =
  typeof navigator !== "undefined" &&
  (/Android/i.test(navigator.userAgent || "") || androidPreview);
