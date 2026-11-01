import { BUTTON_CLASSES, buttonStateClasses } from "../buttonClasses.js";

const PenaltyButton = ({ label, legal, onClick, selected }) => {
  return (
    <button
      className={`${BUTTON_CLASSES} ${buttonStateClasses({ active: selected, legal })}`}
      disabled={!legal}
      onClick={onClick}
    >
      {label}
    </button>
  );
};

export default PenaltyButton;
