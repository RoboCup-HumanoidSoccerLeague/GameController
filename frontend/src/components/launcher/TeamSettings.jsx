import { Field } from "./Field";
import TeamColorSelector from "./TeamColorSelector";
import TeamSelector from "./TeamSelector";

const TeamSettings = ({
  side,
  teams,
  team,
  setTeam,
  isTeamLegal,
  isFieldPlayerColorLegal,
  isGoalkeeperColorLegal,
}) => {
  const setNumber = (number) => {
    const teamOptions = teams.find((t) => t.number === number);
    setTeam({
      ...team,
      number: number,
      fieldPlayerColor: teamOptions.fieldPlayerColors[0],
      goalkeeperColor: teamOptions.goalkeeperColors[0],
    });
  };
  const teamOptions = teams.find((t) => t.number === team.number);
  return (
    <div className="flex flex-1 flex-col justify-evenly gap-2">
      <Field id={`${side}-team`} label="Team">
        <TeamSelector
          id={`${side}-team`}
          teams={teams}
          number={team.number}
          setNumber={setNumber}
          isTeamLegal={isTeamLegal}
        />
      </Field>
      <div className="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)] gap-2">
        <Field id={`${side}-field-player-color`} label="Field Player Color">
          <TeamColorSelector
            id={`${side}-field-player-color`}
            colors={teamOptions.fieldPlayerColors}
            color={team.fieldPlayerColor}
            setColor={(color) => setTeam({ ...team, fieldPlayerColor: color })}
            isColorLegal={isFieldPlayerColorLegal}
          />
        </Field>
        <Field id={`${side}-goalkeeper-color`} label="Goalkeeper Color">
          <TeamColorSelector
            id={`${side}-goalkeeper-color`}
            colors={teamOptions.goalkeeperColors}
            color={team.goalkeeperColor}
            setColor={(color) => setTeam({ ...team, goalkeeperColor: color })}
            isColorLegal={isGoalkeeperColorLegal}
          />
        </Field>
      </div>
    </div>
  );
};

export default TeamSettings;
