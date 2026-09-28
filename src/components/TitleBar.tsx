import { getCurrentWindow } from "@tauri-apps/api/window";
import { useI18n } from "../lib/i18n";

/**
 * Кастомная шапка окна (окно запущено без нативной рамки, `decorations: false`).
 * Без иконки и названия — только область перетаскивания и кнопки свернуть/закрыть.
 */
export function TitleBar() {
  const { t } = useI18n();
  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="titlebar-controls">
        <button
          className="titlebar-btn"
          title={t("common.minimize")}
          onClick={() => getCurrentWindow().minimize()}
        >
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
            <rect x="0" y="4.5" width="10" height="1" fill="currentColor" />
          </svg>
        </button>
        <button
          className="titlebar-btn close"
          title={t("common.close")}
          onClick={() => getCurrentWindow().close()}
        >
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
            <path
              d="M0 0 L10 10 M10 0 L0 10"
              stroke="currentColor"
              strokeWidth="1.2"
            />
          </svg>
        </button>
      </div>
    </div>
  );
}
