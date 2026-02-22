import { Activity, CheckCircle2 } from "lucide-react";
import { useTranslation } from "react-i18next";

interface QueueSummaryCardsProps {
  activeCount: number;
  completedCount: number;
  totalProgress: number;
}

export function QueueSummaryCards({
  activeCount,
  completedCount,
  totalProgress,
}: QueueSummaryCardsProps) {
  const { t } = useTranslation();

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mb-10">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl p-6 relative overflow-hidden group hover:border-blue-500/50 transition-colors">
        <div className="absolute right-0 top-0 p-6 opacity-10 group-hover:opacity-20 transition-opacity">
          <Activity className="w-24 h-24 text-blue-500" />
        </div>
        <p className="text-zinc-400 font-medium mb-1">{t("queue.activeCount")}</p>
        <h3 className="text-4xl font-bold text-white">{activeCount}</h3>
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
        <p className="text-zinc-400 font-medium mb-1">{t("queue.completedCount")}</p>
        <h3 className="text-4xl font-bold text-white">{completedCount}</h3>
        <div className="mt-4 text-sm text-green-400 font-medium">
          {t("queue.readyToView")}
        </div>
      </div>
    </div>
  );
}
