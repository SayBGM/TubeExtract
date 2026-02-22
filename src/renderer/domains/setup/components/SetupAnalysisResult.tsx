import { motion } from "motion/react";
import { Clock, Download, Monitor, Music, PlayCircle, Video } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { AnalysisResult } from "../../../types";
import { cn } from "../../../lib/cn";

interface SetupAnalysisResultProps {
  analysisResult: AnalysisResult;
  selectedMode: "video" | "audio";
  selectedQualityId?: string;
  qualityOptions: AnalysisResult["videoOptions"];
  onSelectMode: (mode: "video" | "audio") => void;
  onSelectQuality: (qualityId: string) => void;
  onOpenVideoInBrowser: () => Promise<void>;
  onEnqueue: () => Promise<void>;
}

export function SetupAnalysisResult({
  analysisResult,
  selectedMode,
  selectedQualityId,
  qualityOptions,
  onSelectMode,
  onSelectQuality,
  onOpenVideoInBrowser,
  onEnqueue,
}: SetupAnalysisResultProps) {
  const { t } = useTranslation();

  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.95 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.95 }}
      transition={{ duration: 0.25 }}
      className="bg-zinc-900 border border-zinc-800 rounded-3xl overflow-hidden shadow-2xl mt-6"
    >
      <div className="grid grid-cols-1 md:grid-cols-3">
        <button
          type="button"
          onClick={() => void onOpenVideoInBrowser()}
          className="relative aspect-video bg-black group overflow-hidden rounded-2xl text-left cursor-pointer"
          aria-label={analysisResult.title}
        >
          <img
            src={analysisResult.thumbnailUrl}
            alt={analysisResult.title}
            className="w-full h-full object-cover opacity-80 group-hover:opacity-100 transition-opacity duration-500"
          />
          <div className="absolute inset-0 flex items-center justify-center">
            <PlayCircle className="w-16 h-16 text-white/80 drop-shadow-lg backdrop-blur-sm rounded-full bg-black/20" />
          </div>
          <div className="absolute bottom-3 right-3 bg-black/80 text-white text-xs font-bold px-2 py-1 rounded backdrop-blur-md flex items-center gap-1">
            <Clock className="w-3 h-3" />
            {t("setup.duration", { seconds: analysisResult.durationSec })}
          </div>
        </button>

        <div className="col-span-2 p-8 flex flex-col justify-between">
          <div>
            <h2 className="text-2xl font-bold text-white mb-2 line-clamp-2 leading-tight">
              {analysisResult.title}
            </h2>
            <p className="text-zinc-400 font-medium mb-8 flex items-center gap-2">
              <span className="w-8 h-8 rounded-full bg-zinc-800 flex items-center justify-center text-xs font-bold text-zinc-300">
                CH
              </span>
              {analysisResult.channel}
            </p>

            <div className="grid grid-cols-2 gap-6 mb-8">
              <div className="space-y-3">
                <label className="text-sm font-semibold text-zinc-500 uppercase tracking-wider">
                  {t("setup.format")}
                </label>
                <div className="flex bg-zinc-950 p-1 rounded-xl border border-zinc-800">
                  <button
                    type="button"
                    onClick={() => onSelectMode("video")}
                    className={cn(
                      "flex-1 flex items-center justify-center gap-2 py-2.5 rounded-lg text-sm font-medium transition-all",
                      selectedMode === "video"
                        ? "bg-zinc-800 text-white shadow-sm"
                        : "text-zinc-500 hover:text-zinc-300",
                    )}
                  >
                    <Video className="w-4 h-4" />
                    {t("setup.video")}
                  </button>
                  <button
                    type="button"
                    onClick={() => onSelectMode("audio")}
                    className={cn(
                      "flex-1 flex items-center justify-center gap-2 py-2.5 rounded-lg text-sm font-medium transition-all",
                      selectedMode === "audio"
                        ? "bg-zinc-800 text-white shadow-sm"
                        : "text-zinc-500 hover:text-zinc-300",
                    )}
                  >
                    <Music className="w-4 h-4" />
                    {t("setup.audio")}
                  </button>
                </div>
              </div>

              <div className="space-y-3">
                <label className="text-sm font-semibold text-zinc-500 uppercase tracking-wider">
                  {selectedMode === "audio"
                    ? t("setup.audioQuality")
                    : t("setup.videoQuality")}
                </label>
                <div className="relative">
                  <Monitor className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-zinc-500" />
                  <select
                    className="w-full bg-zinc-950 border border-zinc-800 text-white text-sm rounded-xl pl-10 pr-4 py-3 focus:outline-none focus:ring-2 focus:ring-blue-500/50 appearance-none"
                    value={selectedQualityId}
                    onChange={(e) => onSelectQuality(e.target.value)}
                  >
                    {qualityOptions.map((quality) => (
                      <option key={quality.id} value={quality.id}>
                        {quality.label}
                      </option>
                    ))}
                  </select>
                </div>
              </div>
            </div>
          </div>

          <div className="flex items-center gap-4 pt-6 border-t border-zinc-800/50">
            <button
              type="button"
              data-testid="setup-download-now-button"
              onClick={() => void onEnqueue()}
              className="w-full bg-red-600 hover:bg-red-700 text-white font-bold py-3.5 rounded-xl flex items-center justify-center gap-2 transition-all shadow-lg shadow-red-900/20 active:scale-95"
            >
              <Download className="w-5 h-5" />
              {t("setup.actions.downloadNow")}
            </button>
          </div>
        </div>
      </div>
    </motion.div>
  );
}
