import { overlay } from "overlay-kit";
import { ConfirmModal } from "../components/ConfirmModal";

interface ConfirmModalOptions {
  title: string;
  description: string;
  confirmText: string;
  cancelText: string;
  secondaryAction?: {
    label: string;
    disabled?: boolean;
    onClick: () => void;
  };
}

export function openConfirmModal(options: ConfirmModalOptions) {
  return overlay.openAsync<boolean>(({ isOpen, close, unmount }) => (
    <ConfirmModal
      title={options.title}
      description={options.description}
      confirmText={options.confirmText}
      cancelText={options.cancelText}
      secondaryAction={options.secondaryAction}
      isOpen={isOpen}
      onConfirm={() => {
        close(true);
        unmount();
      }}
      onCancel={() => {
        close(false);
        unmount();
      }}
    />
  ));
}
