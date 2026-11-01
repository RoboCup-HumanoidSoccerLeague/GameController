import { CheckboxField } from "./Field";
import Section from "./Section";
import TeamSettings from "./TeamSettings";

const GameSettings = ({ teams, game, setGame }) => {
  const mirrored = game.sideMapping === "homeDefendsRightGoal";
  return (
    <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_5rem_minmax(0,1fr)] gap-3">
      {[
        ["home", "away"],
        ["away", "home"],
      ].map(([side, other]) => {
        return (
          <div
            className="flex min-w-0"
            // The middle column is reserved for the mirror checkbox.
            style={{ order: (side === "home") === mirrored ? 3 : 1 }}
            key={side}
          >
            <Section
              title={side === "home" ? "Home" : "Away"}
              action={
                <CheckboxField
                  type="radio"
                  id={`kick-off-${side}`}
                  label="Kick-off"
                  checked={game.kickOffSide === side}
                  onChange={() => setGame({ ...game, kickOffSide: side })}
                />
              }
            >
              <TeamSettings
                side={side}
                teams={teams}
                team={game.teams[side]}
                setTeam={(team) =>
                  setGame({
                    ...game,
                    teams: { ...game.teams, [side]: team },
                  })
                }
                isTeamLegal={game.teams.home.number != game.teams.away.number}
                isFieldPlayerColorLegal={
                  game.teams[side].fieldPlayerColor != game.teams[other].goalkeeperColor &&
                  game.teams.home.fieldPlayerColor != game.teams.away.fieldPlayerColor
                }
                isGoalkeeperColorLegal={
                  game.teams[side].goalkeeperColor != game.teams[other].fieldPlayerColor
                }
              />
            </Section>
          </div>
        );
      })}
      <div className="flex flex-col items-center justify-center gap-1" style={{ order: 2 }}>
        <span aria-hidden="true" className="text-2xl leading-none">
          ⇄
        </span>
        <CheckboxField
          id="mirror"
          label="Mirror"
          title="Swap the sides: the home team starts on the right"
          checked={mirrored}
          onChange={(e) =>
            setGame({
              ...game,
              sideMapping: e.target.checked ? "homeDefendsRightGoal" : "homeDefendsLeftGoal",
            })
          }
        />
      </div>
    </div>
  );
};

export default GameSettings;
