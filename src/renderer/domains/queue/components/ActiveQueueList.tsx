import { useCallback, useMemo, useState } from "react";
import { Activity } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { QueueItem } from "../../../types";
import { ActiveQueueItem } from "./ActiveQueueItem";

interface ActiveQueueListProps {
  activeJobs: QueueItem[];
  onOpenVideoInBrowser: (url: string) => Promise<void>;
  onPauseJob: (jobId: string) => Promise<void>;
  onResumeJob: (jobId: string) => Promise<void>;
  onCancelJob: (jobId: string) => Promise<void>;
}

export function ActiveQueueList({
  activeJobs,
  onOpenVideoInBrowser,
  onPauseJob,
  onResumeJob,
  onCancelJob,
}: ActiveQueueListProps) {
  const { t } = useTranslation();
  const [expandedLogById, setExpandedLogById] = useState<Record<string, boolean>>({});

  const expandedMap = useMemo(() => expandedLogById, [expandedLogById]);

  const toggleLogPanel = useCallback((jobId: string) => {
    setExpandedLogById((prev) => ({
      ...prev,
      [jobId]: !prev[jobId],
    }));
  }, []);

  return (
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
            <ActiveQueueItem
              key={job.id}
              job={job}
              expanded={Boolean(expandedMap[job.id])}
              onToggleLog={toggleLogPanel}
              onOpenVideoInBrowser={onOpenVideoInBrowser}
              onPauseJob={onPauseJob}
              onResumeJob={onResumeJob}
              onCancelJob={onCancelJob}
            />
          ))
        )}
      </div>
    </div>
  );
}
