import { CheckboxField, CONTROL_BOX_CLASSES, Field } from "./Field";

const WindowSettings = ({ window, setWindow }) => {
  return (
    <Field label="Window">
      <div className={`flex items-center border-transparent ${CONTROL_BOX_CLASSES}`}>
        <CheckboxField
          id="fullscreen"
          label="Fullscreen"
          checked={window.fullscreen}
          onChange={(e) => setWindow({ ...window, fullscreen: e.target.checked })}
        />
      </div>
    </Field>
  );
};

export default WindowSettings;
