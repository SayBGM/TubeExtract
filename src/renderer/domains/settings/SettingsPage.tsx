import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FormProvider, useForm } from "react-hook-form";
import { Save } from "lucide-react";
import {
  checkUpdate,
  openExternalUrl,
  pickDownloadDir,
  runDiagnostics,
  setSettings,
} from "../../lib/electronClient";
import { settingsQueries, settingsQueryOptions } from "../../queries";
import { useUIStore } from "../../store/uiStore";
import type { AppSettings } from "../../types";
import { SettingsDefaultsSection } from "./components/SettingsDefaultsSection";
import { SettingsDiagnosticsSection } from "./components/SettingsDiagnosticsSection";
import { SettingsUpdateSection } from "./components/SettingsUpdateSection";

const EMPTY_SETTINGS: AppSettings = {
  downloadDir: "",
  maxRetries: 0,
  language: "ko",
};

export function SettingsPage() {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
  const setToast = useUIStore((state) => state.setToast);
  const [diagnostics, setDiagnostics] = useState<string>("-");
  const [updateMessage, setUpdateMessage] = useState<string>("-");

  const settingsForm = useForm<AppSettings>({
    defaultValues: EMPTY_SETTINGS,
  });

  const settingsQuery = useQuery(settingsQueryOptions.current());

  useEffect(() => {
    if (!settingsQuery.error) return;
    console.error(settingsQuery.error);
    setToast({ type: "error", message: t("common.unknownError") });
  }, [setToast, settingsQuery.error, t]);

  useEffect(() => {
    if (!settingsQuery.data) return;
    settingsForm.reset(settingsQuery.data);
  }, [settingsForm, settingsQuery.data]);

  const saveSettingsMutation = useMutation({
    mutationFn: setSettings,
    onSuccess: async (_response, nextSettings) => {
      queryClient.setQueryData(settingsQueries.current.queryKey, nextSettings);
      settingsForm.reset(nextSettings);
      await i18n.changeLanguage(nextSettings.language);
      setToast({ type: "success", message: t("settings.saved") });
    },
    onError: (error) => {
      console.error(error);
      setToast({ type: "error", message: t("common.unknownError") });
    },
  });

  const diagnosticsMutation = useMutation({
    mutationFn: runDiagnostics,
    onSuccess: (result) => {
      setDiagnostics(result.message);
    },
    onError: (error) => {
      console.error(error);
      setToast({ type: "error", message: t("common.unknownError") });
    },
  });

  const updateMutation = useMutation({
    mutationFn: checkUpdate,
    onSuccess: async (result) => {
      if (!result.hasUpdate) {
        setUpdateMessage(t("settings.update.latest"));
        return;
      }

      setUpdateMessage(
        t("settings.update.available", {
          version: result.latestVersion,
        }),
      );

      if (result.url) {
        await openExternalUrl(result.url);
      }
    },
    onError: (error) => {
      console.error(error);
      setToast({ type: "error", message: t("common.unknownError") });
    },
  });

  const isLoadingSettings = settingsQuery.isPending && !settingsQuery.data;
  const isSaveDisabled =
    isLoadingSettings ||
    saveSettingsMutation.isPending ||
    !settingsForm.formState.isDirty;

  const onSubmit = settingsForm.handleSubmit(async (values) => {
    await saveSettingsMutation.mutateAsync(values);
  });

  const onPickDownloadDir = async () => {
    try {
      const selected = await pickDownloadDir();
      if (!selected) return;
      settingsForm.setValue("downloadDir", selected, { shouldDirty: true });
    } catch (error) {
      console.error(error);
      setToast({ type: "error", message: t("common.unknownError") });
    }
  };

  return (
    <section className="max-w-4xl mx-auto pt-10">
      <h1 className="text-3xl font-bold text-white mb-8">{t("settings.title")}</h1>

      <FormProvider {...settingsForm}>
        <form className="space-y-6" onSubmit={onSubmit}>
          <SettingsDefaultsSection
            isLoading={isLoadingSettings}
            onPickDownloadDir={onPickDownloadDir}
          />

          <SettingsDiagnosticsSection
            diagnostics={diagnostics}
            isPending={diagnosticsMutation.isPending}
            onDiagnose={() => diagnosticsMutation.mutate()}
          />

          <SettingsUpdateSection
            updateMessage={updateMessage}
            isPending={updateMutation.isPending}
            onCheckUpdate={() => updateMutation.mutate()}
          />

          <div className="flex justify-end pt-4">
            <button
              type="submit"
              data-testid="settings-save-button"
              className="bg-blue-600 hover:bg-blue-500 text-white px-8 py-3 rounded-xl font-bold shadow-lg shadow-blue-900/20 flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
              disabled={isSaveDisabled}
            >
              <Save className="w-4 h-4" />
              {t("settings.save")}
            </button>
          </div>
        </form>
      </FormProvider>
    </section>
  );
}
