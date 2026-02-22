import {
  cancelJob,
  clearTerminalJobs,
  deleteFile,
  getQueueSnapshot,
  openFolder,
  pauseJob,
  resumeJob,
} from "../../lib/electronClient";

export async function clearCompletedQueueJobs() {
  await clearTerminalJobs();
  const snapshot = await getQueueSnapshot();
  return snapshot.items;
}

export const queueActions = {
  pauseJob,
  resumeJob,
  cancelJob,
  openFolder,
  deleteFile,
  clearCompletedQueueJobs,
};
