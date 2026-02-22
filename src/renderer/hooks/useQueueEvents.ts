import { useEffect } from "react";
import { getQueueSnapshot, isNativeDesktop, onQueueUpdated } from "../lib/electronClient";
import { useQueueStore } from "../store/queueStore";

const QUEUE_POLLING_INTERVAL_MS = 1000;

export function useQueueEvents() {
  const applyQueueSnapshot = useQueueStore((state) => state.applyQueueSnapshot);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let pollTimer: number | undefined;

    const setup = async () => {
      const snapshot = await getQueueSnapshot();
      applyQueueSnapshot(snapshot.items);

      if (isNativeDesktop()) {
        unlisten = onQueueUpdated((snapshot) => {
          applyQueueSnapshot(snapshot.items);
        });
      } else {
        pollTimer = window.setInterval(() => {
          getQueueSnapshot()
            .then((snapshot) => applyQueueSnapshot(snapshot.items))
            .catch(console.error);
        }, QUEUE_POLLING_INTERVAL_MS);
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
  }, [applyQueueSnapshot]);
}
