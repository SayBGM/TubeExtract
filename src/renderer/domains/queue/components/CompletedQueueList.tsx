import { CheckCircle2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { QueueItem } from "../../../types";
import { CompletedQueueItem } from "./CompletedQueueItem";

interface CompletedQueueListProps {
  completedJobs: QueueItem[];
  onClearTerminalJobs: () => Promise<void>;
  onOpenFolder: (path: string) => Promise<void>;
  onDeleteFile: (path: string) => Promise<void>;
}

export function CompletedQueueList({
  completedJobs,
  onClearTerminalJobs,
  onOpenFolder,
  onDeleteFile,
}: CompletedQueueListProps) {
  const { t } = useTranslation();

  return (
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
              <CompletedQueueItem
                key={job.id}
                job={job}
                onOpenFolder={onOpenFolder}
                onDeleteFile={onDeleteFile}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
