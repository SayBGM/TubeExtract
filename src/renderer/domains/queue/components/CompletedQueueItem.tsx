import { Check, FolderOpen, Trash2 } from "lucide-react";
import type { QueueItem } from "../../../types";

interface CompletedQueueItemProps {
  job: QueueItem;
  onOpenFolder: (path: string) => Promise<void>;
  onDeleteFile: (path: string) => Promise<void>;
}

export function CompletedQueueItem({
  job,
  onOpenFolder,
  onDeleteFile,
}: CompletedQueueItemProps) {
  return (
    <div className="flex items-center gap-4 p-4 hover:bg-zinc-800/30 transition-colors group">
      <div className="w-12 h-12 rounded bg-zinc-950 overflow-hidden shrink-0 relative">
        <div className="w-full h-full bg-zinc-800" />
        <div className="absolute inset-0 flex items-center justify-center">
          <Check className="w-5 h-5 text-green-500 drop-shadow-md" />
        </div>
      </div>
      <div className="flex-1 min-w-0">
        <h4 className="text-sm font-medium text-white truncate">{job.title}</h4>
        <p className="text-xs text-zinc-500 mt-0.5">{job.outputPath ?? "-"}</p>
      </div>
      <div className="flex items-center gap-2 opacity-0 group-hover:opacity-100 transition-opacity">
        <button
          type="button"
          onClick={() => job.outputPath && void onOpenFolder(job.outputPath)}
          className="p-2 rounded-lg hover:bg-zinc-800 text-zinc-400 hover:text-white transition-colors cursor-pointer"
        >
          <FolderOpen className="w-4 h-4" />
        </button>
        <button
          type="button"
          onClick={() => job.outputPath && void onDeleteFile(job.outputPath)}
          className="p-2 rounded-lg hover:bg-red-900/20 text-zinc-400 hover:text-red-500 transition-colors cursor-pointer"
        >
          <Trash2 className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
}
