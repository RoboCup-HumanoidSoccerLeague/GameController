import { useEffect, useState } from "react";
import ActionButton from "./ActionButton";
import PlayerButton from "./PlayerButton";
import * as actions from "../../actions.js";
import { applyAction } from "../../api.js";

const textClasses = {
  red: "text-red-600",
  blue: "text-blue-600",
  yellow: "text-yellow-400",
  black: "text-black",
  white: "text-black",
  green: "text-green-600",
  orange: "text-orange-400",
  purple: "text-purple-600",
  brown: "text-amber-800",
  gray: "text-gray-600",
};

const SELECT_STATE_DEFAULT = 0;
const SELECT_STATE_GOALKEEPER = 1;
const SELECT_STATE_PLAYER_OUT = 2;
const SELECT_STATE_PLAYER_IN = 3;
const SELECT_STATE_PSO_COLOR = 4;
const SELECT_STATE_PSO_PLAYER = 5;

const TeamHeader = ({ color, isKicking, name }) => {
  return (
    <div className="flex items-center justify-center gap-2">
      <svg
        className={`${isKicking ? "" : "invisible"} text-black`}
        fill="currentColor"
        height="14"
        width="14"
      >
        <circle cx="7" cy="7" r="7" />
      </svg>
      <h1 className={`text-center text-2xl font-semibold ${textClasses[color]}`}>{name}</h1>
    </div>
  );
};

const TeamStats = ({ game, side, sign, team }) => {
  return (
    <dl className="flex-1">
      <dt className="sr-only">Score</dt>
      <dd
        className={`font-bold text-4xl ${sign > 0 ? "text-right" : "text-left"} tabular-nums ${
          team.illegalCommunication ? "text-fuchsia-400" : ""
        }`}
      >
        {team.score}
      </dd>

      {game.phase === "penaltyShootout" ? (
        <>
          <dt>Shot{game.kickingSide === side ? "" : "s"}:</dt>
          <dd className="tabular-nums text-right">{team.penaltyShot}</dd>
        </>
      ) : (
        <>
          <dt className={team.illegalCommunication ? "text-fuchsia-400" : ""}>Messages:</dt>
          <dd
            className={`tabular-nums text-right ${
              team.illegalCommunication ? "text-fuchsia-400" : ""
            }`}
          >
            {team.messageBudget}
          </dd>
        </>
      )}

      <dt>Penalties:</dt>
      <dd className="tabular-nums text-right">{team.penaltyCounter}</dd>
    </dl>
  );
};

const FreeKickButton = ({ game, legalTeamActions, side, setPlay, label, action }) => {
  return (
    <ActionButton
      action={{ type: "startSetPlay", args: { side: side, setPlay: setPlay } }}
      active={game.setPlay === setPlay && game.kickingSide === side}
      label={label}
      legal={legalTeamActions[action]}
    />
  );
};

const TeamPanel = ({
  connectionStatus,
  game,
  legalPenaltyActions,
  legalTeamActions,
  params,
  selectedPenaltyCall,
  setSelectedPenaltyCall,
  side,
  sign,
  teamNames,
}) => {
  const team = game.teams[side];
  const teamConnectionStatus = connectionStatus[side];
  const teamParams = params.game.teams[side];

  // The operator can press the shift key to remove penalties early or select the goalkeeper.
  const [alternative, setAlternative] = useState(false);
  useEffect(() => {
    const onKeydown = (e) => {
      if (e.key === "Shift") {
        setAlternative(true);
      }
    };
    const onKeyup = (e) => {
      if (e.key === "Shift") {
        setAlternative(false);
      }
    };
    document.addEventListener("keydown", onKeydown);
    document.addEventListener("keyup", onKeyup);
    return () => {
      document.removeEventListener("keydown", onKeydown);
      document.removeEventListener("keyup", onKeyup);
    };
  });

  // This indicates whether we are currently in the process of substitution or player selection.
  // SELECT_STATE_PLAYER_IN has an additional attribute playerOut which contains the player to be
  // removed, SELECT_STATE_PSO_PLAYER has an additional attribute for the jersey color the player
  // is wearing (goalkeeper / field player).
  const [selectState, setSelectState] = useState({ type: SELECT_STATE_DEFAULT });
  const handlePlayerClick = (player) => {
    switch (selectState.type) {
      case SELECT_STATE_DEFAULT:
        if (selectedPenaltyCall != null) {
          applyAction({
            type: "penalize",
            args: {
              side: side,
              player: player.number,
              call: actions.PENALTIES[selectedPenaltyCall][1],
            },
          });
          if (actions.PENALTIES[selectedPenaltyCall][1] != "motionInSet") {
            setSelectedPenaltyCall(null);
          }
        } else {
          applyAction({
            type: "unpenalize",
            args: { side: side, player: player.number, force: alternative },
          });
        }
        break;
      case SELECT_STATE_GOALKEEPER:
        applyAction({
          type: "selectGoalkeeper",
          args: { side: side, player: player.number },
        });
        setSelectState({ type: SELECT_STATE_DEFAULT });
        break;
      case SELECT_STATE_PLAYER_OUT:
        setSelectState({ type: SELECT_STATE_PLAYER_IN, playerOut: player.number });
        break;
      case SELECT_STATE_PLAYER_IN:
        applyAction({
          type: "substitute",
          args: { side: side, playerOut: selectState.playerOut, playerIn: player.number },
        });
        setSelectState({ type: SELECT_STATE_DEFAULT });
        break;
      case SELECT_STATE_PSO_COLOR:
        // SELECT_STATE_PSO_COLOR doesn't end up here because it has its own handler.
        break;
      case SELECT_STATE_PSO_PLAYER:
        applyAction({
          type: "selectPenaltyShotPlayer",
          args: { side: side, player: player.number, goalkeeper: selectState.goalkeeper },
        });
        setSelectState({ type: SELECT_STATE_DEFAULT });
        break;
    }
  };

  const outerColumn = sign > 0 ? "col-start-1" : "col-start-3";
  const innerColumn = sign > 0 ? "col-start-3" : "col-start-1";

  return (
    <div className="min-w-[290px] flex flex-col gap-2">
      <TeamHeader
        color={teamParams.fieldPlayerColor}
        isKicking={game.kickingSide === side}
        name={teamNames[side]}
      />
      <div className="grid grid-flow-col grid-rows-4 auto-cols-fr gap-2">
        <div className={outerColumn}>
          <ActionButton
            action={() => {
              setSelectState(
                selectState.type != SELECT_STATE_DEFAULT
                  ? { type: SELECT_STATE_DEFAULT }
                  : game.phase === "penaltyShootout"
                    ? teamParams.goalkeeperColor === teamParams.fieldPlayerColor
                      ? // If the goalkeeper doesn't have a special jersey color, its selection can be
                        // skipped.
                        { type: SELECT_STATE_PSO_PLAYER, goalkeeper: game.kickingSide != side }
                      : { type: SELECT_STATE_PSO_COLOR }
                    : alternative
                      ? { type: SELECT_STATE_GOALKEEPER }
                      : { type: SELECT_STATE_PLAYER_OUT },
              );
            }}
            active={selectState.type != SELECT_STATE_DEFAULT}
            label={
              game.phase === "penaltyShootout" ||
              alternative ||
              selectState.type === SELECT_STATE_GOALKEEPER
                ? "Select"
                : "Substitute"
            }
            legal={true}
          />
        </div>
        <div className={outerColumn}>
          <ActionButton
            action={{ type: "timeout", args: { side: side } }}
            label="Timeout"
            legal={legalTeamActions[actions.TIMEOUT]}
          />
        </div>
        <div className={outerColumn}>
          <FreeKickButton
            game={game}
            legalTeamActions={legalTeamActions}
            side={side}
            setPlay="directFreeKick"
            label="Direct Free Kick"
            action={actions.DIRECT_FREE_KICK}
          />
        </div>
        <div className={outerColumn}>
          <FreeKickButton
            game={game}
            legalTeamActions={legalTeamActions}
            side={side}
            setPlay="throwIn"
            label="Throw-in"
            action={actions.THROW_IN}
          />
        </div>
        <div className="col-start-2 row-span-2">
          <ActionButton
            action={{ type: "goal", args: { side: side } }}
            label="Goal"
            legal={legalTeamActions[actions.GOAL]}
          />
        </div>
        <div className="col-start-2">
          <FreeKickButton
            game={game}
            legalTeamActions={legalTeamActions}
            side={side}
            setPlay="indirectFreeKick"
            label="Indirect Free Kick"
            action={actions.INDIRECT_FREE_KICK}
          />
        </div>
        <div className="col-start-2">
          <FreeKickButton
            game={game}
            legalTeamActions={legalTeamActions}
            side={side}
            setPlay="goalKick"
            label="Goal Kick"
            action={actions.GOAL_KICK}
          />
        </div>
        <div className={`${innerColumn} row-span-2`}>
          <TeamStats game={game} side={side} sign={sign} team={team} />
        </div>
        <div className={innerColumn}>
          <FreeKickButton
            game={game}
            legalTeamActions={legalTeamActions}
            side={side}
            setPlay="penaltyKick"
            label="Penalty Kick"
            action={actions.PENALTY_KICK}
          />
        </div>
        <div className={innerColumn}>
          <FreeKickButton
            game={game}
            legalTeamActions={legalTeamActions}
            side={side}
            setPlay="cornerKick"
            label="Corner Kick"
            action={actions.CORNER_KICK}
          />
        </div>
      </div>
      <div className="grow flex flex-col gap-2 overflow-auto">
        {selectState.type === SELECT_STATE_PSO_COLOR
          ? [true, false].map((isGoalkeeper) => (
              <PlayerButton
                key={isGoalkeeper}
                color={isGoalkeeper ? teamParams.goalkeeperColor : teamParams.fieldPlayerColor}
                legal={true}
                sign={sign}
                onClick={() =>
                  setSelectState({ type: SELECT_STATE_PSO_PLAYER, goalkeeper: isGoalkeeper })
                }
                player={null}
              />
            ))
          : team.players
              .map((player, index) => {
                return {
                  ...player,
                  connectionStatus: teamConnectionStatus[index],
                  number: index + 1,
                };
              })
              .filter(
                selectState.type === SELECT_STATE_PSO_PLAYER
                  ? () => true
                  : selectState.type === SELECT_STATE_PLAYER_IN
                    ? (player) => player.penalty === "substitute"
                    : (player) => player.penalty != "substitute",
              )
              .map((player) => (
                <PlayerButton
                  key={player.number}
                  color={
                    (
                      selectState.type === SELECT_STATE_PSO_PLAYER
                        ? selectState.goalkeeper
                        : (selectState.type === SELECT_STATE_PLAYER_IN
                            ? selectState.playerOut
                            : player.number) === team.goalkeeper
                    )
                      ? teamParams.goalkeeperColor
                      : teamParams.fieldPlayerColor
                  }
                  legal={
                    selectState.type != SELECT_STATE_DEFAULT ||
                    actions.isPenaltyCallLegalForPlayer(
                      legalPenaltyActions,
                      side,
                      player.number,
                      selectedPenaltyCall,
                      alternative,
                    )
                  }
                  sign={sign}
                  onClick={() => handlePlayerClick(player)}
                  player={player}
                />
              ))}
      </div>
    </div>
  );
};

export default TeamPanel;
