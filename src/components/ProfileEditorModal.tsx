import { useEffect, useId, useRef, type ReactNode } from "react";
import { useI18n } from "../lib/i18n";

export function ProfileEditorModal({ title, onClose, children }: {
  title: string;
  onClose: () => void;
  children: ReactNode;
}) {
  const { t } = useI18n();
  const titleId = useId();
  const ref = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = ref.current;
    dialog?.showModal();
    return () => dialog?.close();
  }, []);

  return (
    <dialog ref={ref} className="profile-editor-modal" aria-labelledby={titleId}
      onCancel={(event) => { event.preventDefault(); onClose(); }}>
      <div className="profile-editor-head">
        <h2 id={titleId}>{title}</h2>
        <button type="button" aria-label={t("common.close")} onClick={onClose}>✕</button>
      </div>
      <div className="profile-editor-content">{children}</div>
    </dialog>
  );
}
