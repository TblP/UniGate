import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { isWindows } from "../lib/platform";
import { ipc } from "../lib/ipc";
import { useI18n, translate, currentLanguage } from "../lib/i18n";
import { useAppStore } from "../store/useAppStore";

const LATEST_RELEASE_API =
  "https://api.github.com/repos/TblP/UniGate/releases/latest";
const RELEASES_URL = "https://github.com/TblP/UniGate/releases/latest";

type AvailableUpdate = {
  version: string;
  releaseUrl: string;
};

type UpdateState =
  | { phase: "idle" }
  | { phase: "downloading"; progress: number | null }
  | { phase: "stopping" }
  | { phase: "installing" }
  | { phase: "error"; message: string };

function versionParts(version: string): number[] | null {
  const match = version.trim().match(/^v?(\d+)\.(\d+)\.(\d+)(?:[-+].*)?$/i);
  return match ? match.slice(1).map(Number) : null;
}

export function isNewerVersion(candidate: string, current: string): boolean {
  const next = versionParts(candidate);
  const installed = versionParts(current);
  if (!next || !installed) return false;

  for (let i = 0; i < 3; i += 1) {
    if (next[i] !== installed[i]) return next[i] > installed[i];
  }
  return false;
}

async function findAvailableUpdate(signal: AbortSignal): Promise<AvailableUpdate | null> {
  const response = await fetch(LATEST_RELEASE_API, {
    signal,
    cache: "no-store",
    headers: { Accept: "application/vnd.github+json" },
  });
  if (!response.ok) throw new Error(translate(currentLanguage(useAppStore.getState().settings?.language), "update.githubError", { status: response.status }));

  const release = (await response.json()) as {
    tag_name?: string;
  };
  if (!release.tag_name || !isNewerVersion(release.tag_name, __APP_VERSION__)) {
    return null;
  }

  return {
    version: release.tag_name.replace(/^v/i, ""),
    // Не доверяем адресу из API: macOS/Android всегда открывают только
    // заранее зафиксированную официальную страницу репозитория.
    releaseUrl: RELEASES_URL,
  };
}

export function UpdateButton() {
  const { t } = useI18n();
  const [update, setUpdate] = useState<AvailableUpdate | null>(null);
  const [state, setState] = useState<UpdateState>({ phase: "idle" });

  useEffect(() => {
    const controller = new AbortController();
    findAvailableUpdate(controller.signal)
      .then(setUpdate)
      .catch(() => undefined);
    return () => controller.abort();
  }, []);

  if (!update) return null;

  const startUpdate = async () => {
    try {
      if (!isWindows) {
        setState({ phase: "idle" });
        await openUrl(update.releaseUrl);
        return;
      }

      setState({ phase: "downloading", progress: null });
      const { check } = await import("@tauri-apps/plugin-updater");
      const pending = await check({ timeout: 30_000 });
      if (!pending) throw new Error(t("update.unavailable"));

      let downloaded = 0;
      let total: number | undefined;
      await pending.download((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength;
          setState({ phase: "downloading", progress: total ? 0 : null });
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          setState({
            phase: "downloading",
            progress: total ? Math.min(100, Math.round((downloaded / total) * 100)) : null,
          });
        } else if (event.event === "Finished") {
          setState({ phase: "downloading", progress: 100 });
        }
      });
      setState({ phase: "stopping" });
      await ipc.disconnect();
      setState({ phase: "installing" });
      await pending.install();
    } catch (error) {
      setState({
        phase: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  };

  let label = isWindows ? t("update.upgrade", { version: update.version }) : t("update.download", { version: update.version });
  if (state.phase === "downloading") {
    label = state.progress === null ? t("update.downloading") : t("update.progress", { progress: state.progress });
  } else if (state.phase === "stopping") {
    label = t("update.stopping");
  } else if (state.phase === "installing") {
    label = t("update.installing");
  } else if (state.phase === "error") {
    label = t("update.retry");
  }

  const busy =
    state.phase === "downloading" ||
    state.phase === "stopping" ||
    state.phase === "installing";
  return (
    <button
      type="button"
      className="update-button"
      onClick={startUpdate}
      disabled={busy}
      title={state.phase === "error" ? state.message : undefined}
    >
      <span aria-hidden="true">↻</span>
      {label}
    </button>
  );
}
