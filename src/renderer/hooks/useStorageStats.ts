import { useQuery } from "@tanstack/react-query";
import { systemQueryOptions } from "../queries";

export function useStorageStats() {
  const { data } = useQuery(systemQueryOptions.storageStats());
  return data;
}
