export type ModalOptions = {
  onClose: () => void;
  initialFocus?: string;
};

const focusableSelector = [
  "button:not([disabled])",
  "[href]",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
].join(",");

export function modal(node: HTMLElement, options: ModalOptions) {
  let current = options;
  const previousFocus = document.activeElement instanceof HTMLElement
    ? document.activeElement
    : undefined;

  function focusableElements() {
    return [...node.querySelectorAll<HTMLElement>(focusableSelector)]
      .filter((element) => !element.hidden && element.getAttribute("aria-hidden") !== "true");
  }

  function keydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      current.onClose();
      return;
    }
    if (event.key !== "Tab") return;
    const elements = focusableElements();
    if (!elements.length) {
      event.preventDefault();
      node.focus();
      return;
    }
    const first = elements[0];
    const last = elements[elements.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  node.addEventListener("keydown", keydown);
  queueMicrotask(() => {
    const requested = current.initialFocus
      ? node.querySelector<HTMLElement>(current.initialFocus)
      : undefined;
    (requested ?? focusableElements()[0] ?? node).focus();
  });

  return {
    update(options: ModalOptions) {
      current = options;
    },
    destroy() {
      node.removeEventListener("keydown", keydown);
      queueMicrotask(() => previousFocus?.focus());
    },
  };
}
