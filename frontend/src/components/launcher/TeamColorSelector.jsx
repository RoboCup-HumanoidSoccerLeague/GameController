import { selectClasses } from "./Field";

const TeamColorSelector = ({ id, colors, color, setColor, isColorLegal }) => {
  return (
    <select
      className={selectClasses(isColorLegal)}
      id={id}
      value={color}
      onChange={(e) => setColor(e.target.value)}
    >
      {colors.map((color) => (
        <option key={color} value={color}>
          {color}
        </option>
      ))}
    </select>
  );
};

export default TeamColorSelector;
