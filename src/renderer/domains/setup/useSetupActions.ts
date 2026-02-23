import { useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useQueryClient } from "@tanstack/react-query";
import {
  checkDuplicate,
  enqueueJob,
  openFolder,
} from "../../lib/desktopClient";
import { useOpenExternalUrl } from "../../hooks/useOpenExternalUrl";
import { openConfirmModal } from "../../lib/openConfirmModal";
import { setupQueryOptions } from "../../queries";
import { useSetupStore } from "../../store/setupStore";
import { useUIStore } from "../../store/uiStore";

const YOUTUBE_URL_REGEX =
  /^(https?:\/\/)?(www\.)?(youtube\.com\/(watch\?v=|shorts\/|live\/)|youtu\.be\/)[A-Za-z0-9_-]{6,}/i;

function isValidYouTubeUrl(url: string) {
  return YOUTUBE_URL_REGEX.test(url.trim());
}

export function useSetupActions(onSuccessEnqueue: () => void) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const {
    urlInput,
    setAnalyzing,
    setAnalysisResult,
    selectedMode,
    selectedQualityId,
    setAnalyzeError,
    analysisResult,
  } = useSetupStore();
  const setToast = useUIStore((state) => state.setToast);
  const openVideoInBrowser = useOpenExternalUrl();

  const qualityOptions = useMemo(
    () =>
      selectedMode === "video"
        ? (analysisResult?.videoOptions ?? [])
        : (analysisResult?.audioOptions ?? []),
    [analysisResult?.audioOptions, analysisResult?.videoOptions, selectedMode],
  );

  const onOpenVideoInBrowser = useCallback(async () => {
    const targetUrl = analysisResult?.sourceUrl ?? urlInput.trim();
    if (!targetUrl) return;
    await openVideoInBrowser(targetUrl);
  }, [analysisResult?.sourceUrl, openVideoInBrowser, urlInput]);

  const onAnalyze = useCallback(async () => {
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
      const result = await queryClient.fetchQuery(
        setupQueryOptions.analyzeUrl(normalizedUrl),
      );
      setAnalysisResult(result);
    } catch (error) {
      const message =
        error instanceof Error ? error.message : t("common.unknownError");
      setAnalyzeError(message);
    } finally {
      setAnalyzing(false);
    }
  }, [
    queryClient,
    setAnalyzing,
    setAnalyzeError,
    setAnalysisResult,
    t,
    urlInput,
  ]);

  const onEnqueue = useCallback(
    async (forceDuplicate: boolean) => {
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
        onSuccessEnqueue();
      } catch (error) {
        const message =
          error instanceof Error ? error.message : t("common.unknownError");
        setToast({ type: "error", message });
      }
    },
    [
      analysisResult,
      onSuccessEnqueue,
      selectedMode,
      selectedQualityId,
      setToast,
      t,
      urlInput,
    ],
  );

  return {
    qualityOptions,
    onAnalyze,
    onEnqueue,
    onOpenVideoInBrowser,
  };
}
