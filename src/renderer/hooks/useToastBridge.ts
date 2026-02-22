import { useEffect } from "react";
import { toast } from "sonner";
import { useUIStore } from "../store/uiStore";

export function useToastBridge() {
  const queuedToast = useUIStore((state) => state.toast);
  const setToast = useUIStore((state) => state.setToast);

  useEffect(() => {
    if (!queuedToast) return;
    toast[queuedToast.type](queuedToast.message);
    setToast(undefined);
  }, [queuedToast, setToast]);
}
