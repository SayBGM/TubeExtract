import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import { openExternalUrl } from "../lib/desktopClient";
import { useUIStore } from "../store/uiStore";

export function useOpenExternalUrl() {
  const { t } = useTranslation();
  const setToast = useUIStore((state) => state.setToast);

  return useCallback(
    async (url: string) => {
      try {
        await openExternalUrl(url);
      } catch (error) {
        console.error(error);
        const message =
          error instanceof Error ? error.message : t("common.unknownError");
        setToast({ type: "error", message });
      }
    },
    [setToast, t],
  );
}
