import { useState } from "react";
import { QRCodeSVG } from "qrcode.react";

export function ProfileQr({ value, t }: { value: string; t: (key: string) => string }) {
  const [visible, setVisible] = useState(false);
  // Stay below the maximum byte-mode capacity at error correction level M.
  if (new TextEncoder().encode(value).length > 2300) {
    return <p>{t("share.qrTooLong")}</p>;
  }
  return (
    <div className="profile-qr">
      <button type="button" aria-expanded={visible} onClick={() => setVisible(!visible)}>
        {t(visible ? "share.qrHide" : "share.qrShow")}
      </button>
      {visible && (
        <div className="profile-qr-preview">
          <QRCodeSVG value={value} size={240} marginSize={4} level="M" title={t("share.qrTitle")} />
          <p>{t("share.qrSecret")}</p>
        </div>
      )}
    </div>
  );
}
