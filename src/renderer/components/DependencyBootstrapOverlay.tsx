import { LoaderCircle } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { DependencyBootstrapPhase, DependencyBootstrapStatus } from "../types";

interface DependencyBootstrapOverlayProps {
  status: DependencyBootstrapStatus;
}

const PHASE_TRANSLATION_KEY: Record<DependencyBootstrapPhase, string> = {
  idle: "bootstrap.steps.idle",
  preparing: "bootstrap.steps.preparing",
  checking_yt_dlp: "bootstrap.steps.checkingYtDlp",
  downloading_yt_dlp: "bootstrap.steps.downloadingYtDlp",
  checking_ffmpeg: "bootstrap.steps.checkingFfmpeg",
  installing_ffmpeg: "bootstrap.steps.installingFfmpeg",
  ready: "bootstrap.steps.ready",
  failed: "bootstrap.steps.failed",
};

export function DependencyBootstrapOverlay({ status }: DependencyBootstrapOverlayProps) {
  const { t } = useTranslation();

  if (!status.inProgress) {
    return null;
  }

  const progressLabel =
    typeof status.progressPercent === "number"
      ? `${Math.max(0, Math.min(100, Math.round(status.progressPercent)))}%`
      : t("bootstrap.progressUnknown");

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-zinc-950/92 backdrop-blur-sm">
      <div className="w-full max-w-lg rounded-2xl border border-zinc-800 bg-zinc-900/95 p-6 shadow-2xl">
        <div className="flex items-start gap-4">
          <div className="mt-0.5 rounded-xl bg-cyan-500/15 p-2 text-cyan-300">
            <LoaderCircle className="size-5 animate-spin" />
          </div>
          <div className="min-w-0 flex-1">
            <h2 className="text-lg font-semibold text-white">{t("bootstrap.title")}</h2>
            <p className="mt-1 text-sm text-zinc-300">{t("bootstrap.subtitle")}</p>
            <p className="mt-3 text-sm text-zinc-400">
              {t(PHASE_TRANSLATION_KEY[status.phase])}
            </p>
            <div className="mt-3 h-2 overflow-hidden rounded-full bg-zinc-800">
              {typeof status.progressPercent === "number" ? (
                <div
                  className="h-full rounded-full bg-cyan-400 transition-[width] duration-300"
                  style={{ width: `${Math.max(0, Math.min(100, status.progressPercent))}%` }}
                />
              ) : (
                <div className="h-full w-1/3 animate-pulse rounded-full bg-cyan-400/90" />
              )}
            </div>
            <p className="mt-2 text-xs text-zinc-400">{progressLabel}</p>
          </div>
        </div>
      </div>
    </div>
  );
}
