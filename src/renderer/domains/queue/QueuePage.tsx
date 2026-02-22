import { useTranslation } from "react-i18next";
import { useState } from "react";
import {
  Activity,
  ChevronDown,
  ChevronUp,
  Check,
  CheckCircle2,
  FolderOpen,
  Pause,
  Play,
  TerminalSquare,
  Trash2,
  X,
} from "lucide-react";
import {
  cancelJob,
  clearTerminalJobs,
  deleteFile,
  getQueueSnapshot,
  openExternalUrl,
  openFolder,
  pauseJob,
  resumeJob,
} from "../../lib/electronClient";
import { openConfirmModal } from "../../lib/openConfirmModal";
import { useQueueStore } from "../../store/queueStore";
import { useUIStore } from "../../store/uiStore";

export function QueuePage() {
  const { t } = useTranslation();
  const jobs = useQueueStore((state) => state.jobs);
  const applyQueueSnapshot = useQueueStore((state) => state.applyQueueSnapshot);
  const setToast = useUIStore((state) => state.setToast);
  const [expandedLogById, setExpandedLogById] = useState<Record<string, boolean>>({});

  const activeJobs = jobs.filter(
    (job) => !["completed", "failed", "canceled"].includes(job.status),
  );
  const completedJobs = jobs.filter((job) => job.status === "completed");
  const totalProgress = activeJobs.length
    ? activeJobs.reduce((acc, job) => acc + job.progressPercent, 0) /
      activeJobs.length
    : 0;

  const toggleLogPanel = (jobId: string) => {
    setExpandedLogById((prev) => ({
      ...prev,
      [jobId]: !prev[jobId],
    }));
  };

  const onOpenVideoInBrowser = async (url: string) => {
    try {
      await openExternalUrl(url);
    } catch (error) {
      console.error(error);
      setToast({ type: "error", message: t("common.unknownError") });
    }
  };

  const onClearTerminalJobs = async () => {
    if (completedJobs.length === 0) return;

    const confirmed = await openConfirmModal({
      title: t("queue.clearCompleted"),
      description: t("queue.clearCompletedConfirm"),
      confirmText: t("queue.clearCompleted"),
      cancelText: t("common.cancel"),
    });
    if (!confirmed) return;

    try {
      await clearTerminalJobs();
      const snapshot = await getQueueSnapshot();
      applyQueueSnapshot(snapshot.items);
    } catch (error) {
      console.error(error);
      setToast({ type: "error", message: t("common.unknownError") });
    }
  };

  return (
    <section className="max-w-6xl mx-auto pt-8 px-4">
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mb-10">
        <div className="bg-zinc-900 border border-zinc-800 rounded-2xl p-6 relative overflow-hidden group hover:border-blue-500/50 transition-colors">
          <div className="absolute right-0 top-0 p-6 opacity-10 group-hover:opacity-20 transition-opacity">
            <Activity className="w-24 h-24 text-blue-500" />
          </div>
          <p className="text-zinc-400 font-medium mb-1">
            {t("queue.activeCount")}
          </p>
          <h3 className="text-4xl font-bold text-white">{activeJobs.length}</h3>
          <div className="mt-4 flex items-center gap-2 text-sm text-blue-400">
            <div className="w-full bg-zinc-800 h-1.5 rounded-full overflow-hidden">
              <div
                className="bg-blue-500 h-full rounded-full transition-all duration-500"
                style={{ width: `${totalProgress}%` }}
              />
            </div>
            <span>{Math.round(totalProgress)}%</span>
          </div>
        </div>
        <div className="bg-zinc-900 border border-zinc-800 rounded-2xl p-6 relative overflow-hidden group hover:border-green-500/50 transition-colors">
          <div className="absolute right-0 top-0 p-6 opacity-10 group-hover:opacity-20 transition-opacity">
            <CheckCircle2 className="w-24 h-24 text-green-500" />
          </div>
          <p className="text-zinc-400 font-medium mb-1">
            {t("queue.completedCount")}
          </p>
          <h3 className="text-4xl font-bold text-white">
            {completedJobs.length}
          </h3>
          <div className="mt-4 text-sm text-green-400 font-medium">
            {t("queue.readyToView")}
          </div>
        </div>
      </div>

      <div className="mb-12">
        <h2 className="text-xl font-bold text-white mb-6 flex items-center gap-2">
          <Activity className="w-5 h-5 text-blue-500" />
          {t("queue.activeList")}
        </h2>
        <div className="space-y-4">
          {activeJobs.length === 0 ? (
            <div className="text-center py-12 border-2 border-dashed border-zinc-800 rounded-2xl">
              <p className="text-zinc-500">{t("queue.noActiveDownloads")}</p>
            </div>
          ) : (
            activeJobs.map((job) => (
              <div
                key={job.id}
                className="bg-zinc-900 border border-zinc-800 p-5 rounded-2xl shadow-sm flex items-center gap-5 group hover:border-zinc-700 transition-colors"
              >
                <button
                  type="button"
                  onClick={() => void onOpenVideoInBrowser(job.url)}
                  className="w-16 h-16 rounded-lg bg-zinc-950 overflow-hidden shrink-0 cursor-pointer"
                  aria-label={job.title}
                >
                  {job.thumbnailUrl ? (
                    <img
                      src={job.thumbnailUrl}
                      alt={job.title}
                      className="w-full h-full object-cover"
                    />
                  ) : (
                    <div className="w-full h-full bg-zinc-800" />
                  )}
                </button>
                <div className="flex-1 min-w-0">
                  <div className="flex justify-between mb-1">
                    <h4 className="font-semibold text-white truncate pr-4">
                      {job.title}
                    </h4>
                    <span className="text-xs font-mono text-zinc-500">
                      {job.speedText ?? "-"}
                    </span>
                  </div>
                  <div className="w-full bg-zinc-950 h-2 rounded-full overflow-hidden mb-2">
                    <div
                      className="h-full rounded-full transition-all duration-300 bg-blue-600"
                      style={{ width: `${job.progressPercent}%` }}
                    />
                  </div>
                  <div className="flex justify-between text-xs text-zinc-400">
                    <span className="capitalize text-zinc-500">
                      {job.status} • {job.qualityId} • {job.mode}
                    </span>
                    <span>
                      {Math.round(job.progressPercent)}% • ETA:{" "}
                      {job.etaText ?? "-"}
                    </span>
                  </div>
                  <div className="mt-2">
                    <button
                      type="button"
                      onClick={() => toggleLogPanel(job.id)}
                      className="inline-flex items-center gap-1 text-xs text-zinc-400 hover:text-zinc-200 transition-colors cursor-pointer"
                    >
                      <TerminalSquare className="w-3.5 h-3.5" />
                      {t("queue.liveLogs")}
                      {expandedLogById[job.id] ? (
                        <ChevronUp className="w-3.5 h-3.5" />
                      ) : (
                        <ChevronDown className="w-3.5 h-3.5" />
                      )}
                    </button>
                    {expandedLogById[job.id] ? (
                      <div className="mt-2 rounded-lg bg-zinc-950 border border-zinc-800 p-2 max-h-36 overflow-y-auto">
                        {(job.downloadLog ?? []).length === 0 ? (
                          <p className="text-[11px] text-zinc-500">
                            {t("queue.noLogsYet")}
                          </p>
                        ) : (
                          <pre className="text-[11px] leading-4 text-zinc-300 whitespace-pre-wrap break-all font-mono">
                            {(job.downloadLog ?? []).slice(-30).join("\n")}
                          </pre>
                        )}
                      </div>
                    ) : null}
                  </div>
                </div>
                <div className="flex items-center gap-2 pl-4 border-l border-zinc-800">
                  {job.status === "paused" ? (
                    <button
                      onClick={() => void resumeJob(job.id)}
                      className="p-2 rounded-lg hover:bg-zinc-800 text-zinc-400 hover:text-white transition-colors cursor-pointer"
                    >
                      <Play className="w-5 h-5" />
                    </button>
                  ) : (
                    <button
                      onClick={() => void pauseJob(job.id)}
                      className="p-2 rounded-lg hover:bg-zinc-800 text-zinc-400 hover:text-white transition-colors cursor-pointer"
                    >
                      <Pause className="w-5 h-5" />
                    </button>
                  )}
                  <button
                    onClick={() => void cancelJob(job.id)}
                    className="p-2 rounded-lg hover:bg-red-900/20 text-zinc-400 hover:text-red-500 transition-colors cursor-pointer"
                  >
                    <X className="w-5 h-5" />
                  </button>
                </div>
              </div>
            ))
          )}
        </div>
      </div>

      <div>
        <div className="mb-6 flex items-center justify-between gap-4">
          <h2 className="text-xl font-bold text-white flex items-center gap-2">
            <CheckCircle2 className="w-5 h-5 text-green-500" />
            {t("queue.completedList")}
          </h2>
          <button
            type="button"
            onClick={() => void onClearTerminalJobs()}
            disabled={completedJobs.length === 0}
            className="rounded-lg border border-zinc-700 px-3 py-1.5 text-xs text-zinc-300 hover:bg-zinc-800 disabled:opacity-50 disabled:cursor-not-allowed transition-colors cursor-pointer"
          >
            {t("queue.clearCompleted")}
          </button>
        </div>
        <div className="bg-zinc-900 border border-zinc-800 rounded-2xl overflow-hidden">
          {completedJobs.length === 0 ? (
            <div className="text-center py-12 text-zinc-600">
              {t("queue.noDownloadHistory")}
            </div>
          ) : (
            <div className="divide-y divide-zinc-800">
              {completedJobs.map((job) => (
                <div
                  key={job.id}
                  className="flex items-center gap-4 p-4 hover:bg-zinc-800/30 transition-colors group"
                >
                  <div className="w-12 h-12 rounded bg-zinc-950 overflow-hidden shrink-0 relative">
                    <div className="w-full h-full bg-zinc-800" />
                    <div className="absolute inset-0 flex items-center justify-center">
                      <Check className="w-5 h-5 text-green-500 drop-shadow-md" />
                    </div>
                  </div>
                  <div className="flex-1 min-w-0">
                    <h4 className="text-sm font-medium text-white truncate">
                      {job.title}
                    </h4>
                    <p className="text-xs text-zinc-500 mt-0.5">
                      {job.outputPath ?? "-"}
                    </p>
                  </div>
                  <div className="flex items-center gap-2 opacity-0 group-hover:opacity-100 transition-opacity">
                    <button
                      onClick={() =>
                        job.outputPath && void openFolder(job.outputPath)
                      }
                      className="p-2 rounded-lg hover:bg-zinc-800 text-zinc-400 hover:text-white transition-colors cursor-pointer"
                    >
                      <FolderOpen className="w-4 h-4" />
                    </button>
                    <button
                      onClick={() =>
                        job.outputPath && void deleteFile(job.outputPath)
                      }
                      className="p-2 rounded-lg hover:bg-red-900/20 text-zinc-400 hover:text-red-500 transition-colors cursor-pointer"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
