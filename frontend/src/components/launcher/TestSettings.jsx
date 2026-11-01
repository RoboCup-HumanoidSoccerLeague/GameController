import { CheckboxField, Details } from "./Field";

const TestSettings = ({ test, setTest }) => {
  return (
    <Details summary="Testing">
      <CheckboxField
        id="no-delay"
        label="No Delay"
        checked={test.noDelay}
        onChange={(e) => setTest({ ...test, noDelay: e.target.checked })}
      />
      <CheckboxField
        id="penalty-shootout"
        label="Penalty Shoot-out"
        checked={test.penaltyShootout}
        onChange={(e) => setTest({ ...test, penaltyShootout: e.target.checked })}
      />
    </Details>
  );
};

export default TestSettings;
