import { useEffect } from "react";
import { getQueueSnapshot, isNativeDesktop, onQueueUpdated } from "../lib/desktopClient";
import { useQueueStore } from "../store/queueStore";

const WEB_QUEUE_POLLING_INTERVAL_MS = 300;

export function useQueueEvents() {
  const applyQueueSnapshot = useQueueStore((state) => state.applyQueueSnapshot);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let pollTimer: number | undefined;
    const nativeDesktop = isNativeDesktop();

    const setup = async () => {
      if (nativeDesktop) {
        unlisten = onQueueUpdated((snapshot) => {
          applyQueueSnapshot(snapshot.items);
        });

        // Initial hydration once, then rely on event stream only.
        const snapshot = await getQueueSnapshot();
        applyQueueSnapshot(snapshot.items);
        return;
      }

      const syncSnapshot = () => {
        getQueueSnapshot()
          .then((nextSnapshot) => applyQueueSnapshot(nextSnapshot.items))
          .catch(console.error);
      };

      syncSnapshot();
      pollTimer = window.setInterval(syncSnapshot, WEB_QUEUE_POLLING_INTERVAL_MS);
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
