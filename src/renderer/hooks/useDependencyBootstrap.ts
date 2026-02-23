import { useEffect, useState } from "react";
import {
  getDependencyBootstrapStatus,
  isNativeDesktop,
  onDependencyBootstrapUpdated,
} from "../lib/desktopClient";
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
    let pollTimer: number | undefined;

    const setup = async () => {
      const initialStatus = await getDependencyBootstrapStatus();
      setStatus(initialStatus);
      unlisten = onDependencyBootstrapUpdated((nextStatus) => {
        setStatus(nextStatus);
      });
      if (isNativeDesktop() && !unlisten) {
        pollTimer = window.setInterval(() => {
          getDependencyBootstrapStatus().then(setStatus).catch(console.error);
        }, 1000);
      }
    };

    setup().catch(console.error);

    return () => {
      if (unlisten) {
        unlisten();
      }
      if (pollTimer) {
        clearInterval(pollTimer);
      }
    };
  }, []);

  return status;
}
