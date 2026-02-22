import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { AnimatePresence, motion } from "motion/react";
import {
  Clock,
  Download,
  Loader2,
  Monitor,
  Music,
  PlayCircle,
  Search,
  Video,
} from "lucide-react";
import {
  analyzeUrl,
  checkDuplicate,
  enqueueJob,
  openExternalUrl,
  openFolder,
} from "../../lib/electronClient";
import { openConfirmModal } from "../../lib/openConfirmModal";
import { useSetupStore } from "../../store/setupStore";
import { useUIStore } from "../../store/uiStore";
import { cn } from "../../lib/cn";

const YOUTUBE_URL_REGEX =
  /^(https?:\/\/)?(www\.)?(youtube\.com\/(watch\?v=|shorts\/|live\/)|youtu\.be\/)[A-Za-z0-9_-]{6,}/i;

function isValidYouTubeUrl(url: string) {
  return YOUTUBE_URL_REGEX.test(url.trim());
}

export function SetupPage() {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const {
    urlInput,
    setUrlInput,
    isAnalyzing,
    setAnalyzing,
    analysisResult,
    setAnalysisResult,
    selectedMode,
    selectedQualityId,
    setSelectedMode,
    setSelectedQualityId,
    setAnalyzeError,
    analyzeError,
  } = useSetupStore();
  const setToast = useUIStore((state) => state.setToast);
  const onOpenVideoInBrowser = async () => {
    const targetUrl = analysisResult?.sourceUrl ?? urlInput.trim();
    if (!targetUrl) return;
    try {
      await openExternalUrl(targetUrl);
    } catch (error) {
      const message =
        error instanceof Error ? error.message : t("common.unknownError");
      setToast({ type: "error", message });
    }
  };


  const qualityOptions =
    selectedMode === "video"
      ? (analysisResult?.videoOptions ?? [])
      : (analysisResult?.audioOptions ?? []);

  const onAnalyze = async () => {
    const normalizedUrl = urlInput.trim();
    if (!normalizedUrl) {
      setAnalyzeError(t("setup.errors.emptyUrl"));
      return;
    }
    if (!isValidYouTubeUrl(normalizedUrl)) {
      setAnalyzeError(t("setup.errors.invalidUrl"));
      return;
    }

    try {
      setAnalyzing(true);
      setAnalyzeError(undefined);
      const result = await analyzeUrl(normalizedUrl);
      setAnalysisResult(result);
    } catch (error) {
      const message =
        error instanceof Error ? error.message : t("common.unknownError");
      setAnalyzeError(message);
    } finally {
      setAnalyzing(false);
    }
  };

  const onEnqueue = async (forceDuplicate: boolean) => {
    if (!analysisResult || !selectedQualityId) {
      setToast({ type: "error", message: t("setup.errors.selectQuality") });
      return;
    }

    try {
      let shouldForceDuplicate = forceDuplicate;
      if (!shouldForceDuplicate) {
        const duplicate = await checkDuplicate({
          url: urlInput,
          mode: selectedMode,
          qualityId: selectedQualityId,
        });
        if (duplicate.isDuplicate) {
          const existingOutputPath = duplicate.existingOutputPath;

          const confirmed = await openConfirmModal({
            title: t("setup.duplicate.title"),
            description: t("setup.duplicate.description"),
            confirmText: t("setup.duplicate.forceSave"),
            cancelText: t("common.cancel"),
            secondaryAction: {
              label: t("setup.duplicate.openFolder"),
              disabled: !existingOutputPath,
              onClick: () => {
                if (existingOutputPath) {
                  void openFolder(existingOutputPath);
                }
              },
            },
          });
          if (!confirmed) return;
          shouldForceDuplicate = true;
        }
      }

      await enqueueJob({
        url: urlInput.trim(),
        title: analysisResult.title,
        thumbnailUrl: analysisResult.thumbnailUrl,
        mode: selectedMode,
        qualityId: selectedQualityId,
        forceDuplicate: shouldForceDuplicate,
      });
      setToast({ type: "success", message: t("setup.toast.addedToQueue") });
      navigate("/queue");
    } catch (error) {
      const message =
        error instanceof Error ? error.message : t("common.unknownError");
      setToast({ type: "error", message });
    }
  };

  return (
    <section className="max-w-4xl mx-auto pt-10">
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        className="text-center mb-12"
      >
        <h1 className="text-4xl font-bold tracking-tight mb-3 bg-linear-to-r from-white to-zinc-400 bg-clip-text text-transparent">
          {t("setup.title")}
        </h1>
        <p className="text-zinc-500 text-lg">{t("setup.subtitle")}</p>
      </motion.div>

      <form
        onSubmit={(event) => {
          event.preventDefault();
          void onAnalyze();
        }}
        className="bg-zinc-900/50 border border-zinc-800 p-2 rounded-2xl flex items-center shadow-xl shadow-black/20 mb-2 relative z-10"
      >
        <div className="pl-4 pr-3 text-zinc-500">
          <Search className="w-6 h-6" />
        </div>
        <input
          data-testid="setup-url-input"
          type="text"
          value={urlInput}
          onChange={(e) => setUrlInput(e.target.value)}
          placeholder={t("setup.urlPlaceholder")}
          className="bg-transparent border-none text-white text-lg w-full focus:outline-none placeholder:text-zinc-600 py-3 flex-1"
        />
        <button
          data-testid="setup-analyze-button"
          type="submit"
          disabled={isAnalyzing || !urlInput.trim()}
          className={cn(
            "px-6 py-3 rounded-xl font-semibold transition-all duration-200",
            isAnalyzing || !urlInput.trim()
              ? "bg-zinc-800 text-zinc-500 cursor-not-allowed"
              : "bg-blue-600 hover:bg-blue-500 text-white shadow-lg shadow-blue-900/20",
          )}
        >
          {isAnalyzing ? (
            <Loader2 className="w-5 h-5 animate-spin" />
          ) : (
            t("setup.actions.analyze")
          )}
        </button>
      </form>
      {analyzeError ? (
        <p className="text-rose-400 text-sm mt-2">{analyzeError}</p>
      ) : null}
      {isAnalyzing ? (
        <div className="bg-zinc-900 border border-zinc-800 rounded-2xl p-6 mt-4">
          <div className="flex items-center gap-3 text-zinc-300 mb-4">
            <Loader2 className="w-5 h-5 animate-spin text-blue-400" />
            <span className="font-medium">{t("setup.actions.analyzing")}</span>
          </div>
          <div className="space-y-3 animate-pulse">
            <div className="h-40 w-full rounded-xl bg-zinc-800" />
            <div className="h-5 w-2/3 rounded bg-zinc-800" />
            <div className="h-4 w-1/2 rounded bg-zinc-800" />
          </div>
        </div>
      ) : null}

      <AnimatePresence>
        {analysisResult ? (
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
                          onClick={() => setSelectedMode("video")}
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
                          onClick={() => setSelectedMode("audio")}
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
                          onChange={(e) => setSelectedQualityId(e.target.value)}
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
                    data-testid="setup-download-now-button"
                    onClick={() => void onEnqueue(false)}
                    className="w-full bg-red-600 hover:bg-red-700 text-white font-bold py-3.5 rounded-xl flex items-center justify-center gap-2 transition-all shadow-lg shadow-red-900/20 active:scale-95"
                  >
                    <Download className="w-5 h-5" />
                    {t("setup.actions.downloadNow")}
                  </button>
                </div>
              </div>
            </div>
          </motion.div>
        ) : null}
      </AnimatePresence>

    </section>
  );
}
