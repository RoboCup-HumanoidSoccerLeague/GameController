import { applyAction } from "../../api.js";
import { BUTTON_CLASSES, buttonStateClasses } from "../buttonClasses.js";

const ActionButton = ({ action, active, label, legal }) => {
  return (
    <button
      className={`w-full h-full px-2 py-1 ${BUTTON_CLASSES} ${buttonStateClasses({ active, legal })}`}
      disabled={!legal}
      onClick={
        action ? (typeof action === "function" ? action : () => applyAction(action)) : () => {}
      }
    >
      {label}
    </button>
  );
};

export default ActionButton;
