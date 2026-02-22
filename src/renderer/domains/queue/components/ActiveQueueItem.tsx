import {
  ChevronDown,
  ChevronUp,
  Pause,
  Play,
  TerminalSquare,
  X,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import type { QueueItem } from "../../../types";

const POLLING_DISPLAY_LOG_LINE_LIMIT = 30;

interface ActiveQueueItemProps {
  job: QueueItem;
  expanded: boolean;
  onToggleLog: (jobId: string) => void;
  onOpenVideoInBrowser: (url: string) => Promise<void>;
  onPauseJob: (jobId: string) => Promise<void>;
  onResumeJob: (jobId: string) => Promise<void>;
  onCancelJob: (jobId: string) => Promise<void>;
}

export function ActiveQueueItem({
  job,
  expanded,
  onToggleLog,
  onOpenVideoInBrowser,
  onPauseJob,
  onResumeJob,
  onCancelJob,
}: ActiveQueueItemProps) {
  const { t } = useTranslation();

  return (
    <div className="bg-zinc-900 border border-zinc-800 p-5 rounded-2xl shadow-sm flex items-center gap-5 group hover:border-zinc-700 transition-colors">
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
          <h4 className="font-semibold text-white truncate pr-4">{job.title}</h4>
          <span className="text-xs font-mono text-zinc-500">{job.speedText ?? "-"}</span>
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
            {Math.round(job.progressPercent)}% • ETA: {job.etaText ?? "-"}
          </span>
        </div>
        <div className="mt-2">
          <button
            type="button"
            onClick={() => onToggleLog(job.id)}
            className="inline-flex items-center gap-1 text-xs text-zinc-400 hover:text-zinc-200 transition-colors cursor-pointer"
          >
            <TerminalSquare className="w-3.5 h-3.5" />
            {t("queue.liveLogs")}
            {expanded ? (
              <ChevronUp className="w-3.5 h-3.5" />
            ) : (
              <ChevronDown className="w-3.5 h-3.5" />
            )}
          </button>
          {expanded ? (
            <div className="mt-2 rounded-lg bg-zinc-950 border border-zinc-800 p-2 max-h-36 overflow-y-auto">
              {(job.downloadLog ?? []).length === 0 ? (
                <p className="text-[11px] text-zinc-500">{t("queue.noLogsYet")}</p>
              ) : (
                <pre className="text-[11px] leading-4 text-zinc-300 whitespace-pre-wrap break-all font-mono">
                  {(job.downloadLog ?? [])
                    .slice(-POLLING_DISPLAY_LOG_LINE_LIMIT)
                    .join("\n")}
                </pre>
              )}
            </div>
          ) : null}
        </div>
      </div>
      <div className="flex items-center gap-2 pl-4 border-l border-zinc-800">
        {job.status === "paused" ? (
          <button
            type="button"
            onClick={() => void onResumeJob(job.id)}
            className="p-2 rounded-lg hover:bg-zinc-800 text-zinc-400 hover:text-white transition-colors cursor-pointer"
          >
            <Play className="w-5 h-5" />
          </button>
        ) : (
          <button
            type="button"
            onClick={() => void onPauseJob(job.id)}
            className="p-2 rounded-lg hover:bg-zinc-800 text-zinc-400 hover:text-white transition-colors cursor-pointer"
          >
            <Pause className="w-5 h-5" />
          </button>
        )}
        <button
          type="button"
          onClick={() => void onCancelJob(job.id)}
          className="p-2 rounded-lg hover:bg-red-900/20 text-zinc-400 hover:text-red-500 transition-colors cursor-pointer"
        >
          <X className="w-5 h-5" />
        </button>
      </div>
    </div>
  );
}
