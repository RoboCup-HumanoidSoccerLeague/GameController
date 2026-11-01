import { Field, selectClasses } from "./Field";

const CompetitionSettings = ({ competitions, competition, setCompetition }) => {
  return (
    <Field id="competition" label="Competition">
      <select
        className={selectClasses()}
        value={competition.id}
        id="competition"
        onChange={(e) => setCompetition({ ...competition, id: e.target.value })}
      >
        {competitions.map((competition) => (
          <option key={competition.id} value={competition.id}>
            {competition.name}
          </option>
        ))}
      </select>
    </Field>
  );
};

export default CompetitionSettings;
