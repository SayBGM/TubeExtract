import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useOpenExternalUrl } from "../../hooks/useOpenExternalUrl";
import { openConfirmModal } from "../../lib/openConfirmModal";
import { useQueueStore } from "../../store/queueStore";
import { useUIStore } from "../../store/uiStore";
import { TERMINAL_JOB_STATUSES, type JobStatus } from "../../types";
import { queueActions } from "./queueActions";
import { QueueSummaryCards } from "./components/QueueSummaryCards";
import { ActiveQueueList } from "./components/ActiveQueueList";
import { CompletedQueueList } from "./components/CompletedQueueList";

function isTerminalStatus(status: JobStatus) {
  return (TERMINAL_JOB_STATUSES as readonly JobStatus[]).includes(status);
}

export function QueuePage() {
  const { t } = useTranslation();
  const jobs = useQueueStore((state) => state.jobs);
  const applyQueueSnapshot = useQueueStore((state) => state.applyQueueSnapshot);
  const setToast = useUIStore((state) => state.setToast);
  const openVideoInBrowser = useOpenExternalUrl();

  const activeJobs = useMemo(
    () => jobs.filter((job) => !isTerminalStatus(job.status)),
    [jobs],
  );
  const completedJobs = useMemo(
    () => jobs.filter((job) => job.status === "completed"),
    [jobs],
  );
  const totalProgress = useMemo(
    () =>
      activeJobs.length
        ? activeJobs.reduce((acc, job) => acc + job.progressPercent, 0) /
          activeJobs.length
        : 0,
    [activeJobs],
  );

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
      const items = await queueActions.clearCompletedQueueJobs();
      applyQueueSnapshot(items);
    } catch (error) {
      console.error(error);
      setToast({ type: "error", message: t("common.unknownError") });
    }
  };

  return (
    <section className="max-w-6xl mx-auto pt-8 px-4">
      <QueueSummaryCards
        activeCount={activeJobs.length}
        completedCount={completedJobs.length}
        totalProgress={totalProgress}
      />

      <ActiveQueueList
        activeJobs={activeJobs}
        onOpenVideoInBrowser={openVideoInBrowser}
        onPauseJob={queueActions.pauseJob}
        onResumeJob={queueActions.resumeJob}
        onCancelJob={queueActions.cancelJob}
      />

      <CompletedQueueList
        completedJobs={completedJobs}
        onClearTerminalJobs={onClearTerminalJobs}
        onOpenFolder={queueActions.openFolder}
        onDeleteFile={queueActions.deleteFile}
      />
    </section>
  );
}
