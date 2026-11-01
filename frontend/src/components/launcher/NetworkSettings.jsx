import { CheckboxField, Details, Field, selectClasses } from "./Field";

export const InterfaceSettings = ({ interfaces, network, setNetwork }) => {
  return (
    <Field id="interface" label="Network Interface">
      <select
        className={selectClasses()}
        id="interface"
        value={network.interface}
        onChange={(e) => setNetwork({ ...network, interface: e.target.value })}
      >
        {interfaces.map((interphase) => (
          <option key={interphase.id} value={interphase.id}>
            {interphase.id}
          </option>
        ))}
      </select>
    </Field>
  );
};

export const CastingSettings = ({ network, setNetwork }) => {
  return (
    <Details summary="Casting (Advanced)">
      <CheckboxField
        id="broadcast"
        label="Broadcast"
        checked={network.broadcast}
        onChange={(e) => setNetwork({ ...network, broadcast: e.target.checked })}
      />
      <CheckboxField
        id="multicast"
        label="Multicast"
        checked={network.multicast}
        onChange={(e) => setNetwork({ ...network, multicast: e.target.checked })}
      />
    </Details>
  );
};
