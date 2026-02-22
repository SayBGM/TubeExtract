import { contextBridge, ipcRenderer } from "electron";

contextBridge.exposeInMainWorld("electronAPI", {
  invoke(command, args) {
    return ipcRenderer.invoke(command, args);
  },
  onQueueUpdated(listener) {
    const wrapped = (_event, payload) => listener(payload);
    ipcRenderer.on("queue-updated", wrapped);
    return () => {
      ipcRenderer.removeListener("queue-updated", wrapped);
    };
  },
  onDependencyBootstrapUpdated(listener) {
    const wrapped = (_event, payload) => listener(payload);
    ipcRenderer.on("dependency-bootstrap-updated", wrapped);
    return () => {
      ipcRenderer.removeListener("dependency-bootstrap-updated", wrapped);
    };
  },
});
