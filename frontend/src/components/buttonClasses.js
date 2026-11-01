// The look shared by all buttons of the application (launcher and main window).
export const BUTTON_CLASSES = "rounded-md border border-gray-600";

// The additional classes of a button depending on its state: highlighted if active (or selected),
// grayed out if not legal.
export const buttonStateClasses = ({ active = false, legal = true }) =>
  active ? "bg-gray-300" : legal ? "" : "text-gray-500 bg-gray-100";
