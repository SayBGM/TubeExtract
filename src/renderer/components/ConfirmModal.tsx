import { useEffect, useId, useRef } from "react";

interface ConfirmModalProps {
  title: string;
  description: string;
  confirmText: string;
  cancelText: string;
  isOpen: boolean;
  onConfirm: () => void;
  onCancel: () => void;
  secondaryAction?: {
    label: string;
    disabled?: boolean;
    onClick: () => void;
  };
}

export function ConfirmModal({
  title,
  description,
  confirmText,
  cancelText,
  isOpen,
  onConfirm,
  onCancel,
  secondaryAction,
}: ConfirmModalProps) {
  const confirmButtonRef = useRef<HTMLButtonElement>(null);
  const titleId = useId();
  const descriptionId = useId();

  useEffect(() => {
    if (!isOpen) return;
    confirmButtonRef.current?.focus();

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onCancel();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [isOpen, onCancel]);

  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-black/70 p-4"
      role="presentation"
      onClick={onCancel}
    >
      <div
        className="w-full max-w-md rounded-xl border border-zinc-700 bg-zinc-900 p-5 text-white shadow-2xl"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
        onClick={(event) => event.stopPropagation()}
      >
        <h3 id={titleId} className="text-lg font-semibold">{title}</h3>
        <p id={descriptionId} className="mt-2 text-sm text-zinc-300">{description}</p>
        <div className="mt-5 flex flex-wrap justify-end gap-2">
          <button
            type="button"
            onClick={onCancel}
            className="rounded-lg border border-zinc-700 px-4 py-2 text-sm text-zinc-200 hover:bg-zinc-800 transition-colors"
          >
            {cancelText}
          </button>
          {secondaryAction ? (
            <button
              type="button"
              onClick={secondaryAction.onClick}
              disabled={secondaryAction.disabled}
              className="rounded-lg border border-zinc-700 px-4 py-2 text-sm text-zinc-200 hover:bg-zinc-800 disabled:cursor-not-allowed disabled:opacity-50 transition-colors"
            >
              {secondaryAction.label}
            </button>
          ) : null}
          <button
            type="button"
            onClick={onConfirm}
            ref={confirmButtonRef}
            className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-semibold text-white hover:bg-blue-500 transition-colors"
          >
            {confirmText}
          </button>
        </div>
      </div>
    </div>
  );
}
