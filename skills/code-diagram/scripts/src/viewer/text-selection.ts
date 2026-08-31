export function hasTextSelectionWithin(element: Element): boolean {
  const selection = element.ownerDocument.getSelection();
  if (!selection || selection.isCollapsed || selection.rangeCount === 0) {
    return false;
  }

  return element.contains(selection.anchorNode) || element.contains(selection.focusNode);
}
