import { useEffect, useState } from "react";
import {
  getDependencyBootstrapStatus,
  onDependencyBootstrapUpdated,
} from "../lib/electronClient";
import type { DependencyBootstrapStatus } from "../types";

const READY_STATUS: DependencyBootstrapStatus = {
  inProgress: false,
  phase: "ready",
  progressPercent: 100,
  errorMessage: undefined,
};

export function useDependencyBootstrap() {
  const [status, setStatus] = useState<DependencyBootstrapStatus>(READY_STATUS);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setup = async () => {
      const initialStatus = await getDependencyBootstrapStatus();
      setStatus(initialStatus);
      unlisten = onDependencyBootstrapUpdated((nextStatus) => {
        setStatus(nextStatus);
      });
    };

    setup().catch(console.error);

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  return status;
}
