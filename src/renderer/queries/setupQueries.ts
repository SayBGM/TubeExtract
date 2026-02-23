import { createQueryKeys } from "@lukemorales/query-key-factory";
import { queryOptions } from "@tanstack/react-query";
import { analyzeUrl } from "../lib/desktopClient";

const THIRTY_MINUTES_MS = 30 * 60 * 1000;
const TWO_HOURS_MS = 2 * 60 * 60 * 1000;

export const setupQueries = createQueryKeys("setup", {
  analyzeUrl: (url: string) => ({
    queryKey: [url],
    queryFn: () => analyzeUrl(url),
  }),
});

export const setupQueryOptions = {
  analyzeUrl: (url: string) =>
    queryOptions({
      ...setupQueries.analyzeUrl(url),
      staleTime: THIRTY_MINUTES_MS,
      gcTime: TWO_HOURS_MS,
    }),
};
