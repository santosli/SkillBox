export function closeOnBackdropClick(event, onClose) {
  if (event?.target === event?.currentTarget) {
    onClose();
  }
}
