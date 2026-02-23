import { createQueryKeys } from "@lukemorales/query-key-factory";
import { queryOptions } from "@tanstack/react-query";
import { getSettings } from "../lib/desktopClient";

const FIVE_MINUTES_MS = 5 * 60 * 1000;

export const settingsQueries = createQueryKeys("settings", {
  current: {
    queryKey: null,
    queryFn: getSettings,
  },
});

export const settingsQueryOptions = {
  current: () =>
    queryOptions({
      ...settingsQueries.current,
      staleTime: FIVE_MINUTES_MS,
      refetchOnMount: "always",
    }),
};
