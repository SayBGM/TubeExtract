import { createQueryKeys } from "@lukemorales/query-key-factory";
import { queryOptions } from "@tanstack/react-query";
import { getStorageStats } from "../lib/desktopClient";

const STORAGE_STATS_REFETCH_INTERVAL_MS = 10 * 1000;

export const systemQueries = createQueryKeys("system", {
  storageStats: {
    queryKey: null,
    queryFn: getStorageStats,
  },
});

export const systemQueryOptions = {
  storageStats: () =>
    queryOptions({
      ...systemQueries.storageStats,
      staleTime: STORAGE_STATS_REFETCH_INTERVAL_MS,
      refetchInterval: STORAGE_STATS_REFETCH_INTERVAL_MS,
    }),
};
