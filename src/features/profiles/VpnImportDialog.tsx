import { useState, type ComponentProps } from "react";
import { useI18n } from "../../lib/i18n";
import { SubscriptionsPanel } from "../subscriptions/SubscriptionsPanel";
import { ImportDialog } from "./ImportDialog";

export function VpnImportDialog(props: ComponentProps<typeof ImportDialog>) {
  const { t } = useI18n();
  const [mode, setMode] = useState<"server" | "subscription">("server");
  return (
    <div className="vpn-import-content">
      <div className="profile-actions vpn-import-tabs" role="group" aria-label={t("vpn.importMode")}>
        <button type="button" aria-pressed={mode === "server"} onClick={() => setMode("server")}>
          {t("common.server")}
        </button>
        <button type="button" aria-pressed={mode === "subscription"} onClick={() => setMode("subscription")}>
          {t("subscriptions.defaultName")}
        </button>
      </div>
      <div hidden={mode !== "server"}><ImportDialog {...props} /></div>
      <div hidden={mode !== "subscription"}>
        <SubscriptionsPanel embedded onClose={props.onCancel} scanQr={props.scanQr} />
      </div>
    </div>
  );
}
