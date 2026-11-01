import { selectClasses } from "./Field";

const TeamSelector = ({ id, teams, number, setNumber, isTeamLegal }) => {
  return (
    <select
      className={selectClasses(isTeamLegal)}
      id={id}
      value={number}
      onChange={(e) => setNumber(parseInt(e.target.value))}
    >
      {teams
        .sort((a, b) => a.name.localeCompare(b.name))
        .map((team) => (
          <option key={team.number} value={team.number}>
            {team.name} ({team.number})
          </option>
        ))}
    </select>
  );
};

export default TeamSelector;
