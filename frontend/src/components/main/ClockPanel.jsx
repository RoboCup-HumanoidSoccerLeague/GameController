import ActionButton from "./ActionButton";
import * as actions from "../../actions.js";
import { formatMMSS } from "../../utils.js";

const getPhaseDescription = (game) => {
  switch (game.phase) {
    case "firstHalf":
      return game.state === "finished" ? "Half-Time Break" : "First Half";
    case "secondHalf":
      return game.state === "initial" ? "Half-Time Break" : "Second Half";
    case "firstExtraHalf":
      return game.state === "finished" ? "Extra Time" : "First Extra Half";
    case "secondExtraHalf":
      return game.state === "initial" ? "Extra Time" : "Second Extra Half";
    case "penaltyShootout":
      return "Penalty Shoot-out";
  }
  return "";
};

const getStateDescription = (game) => {
  switch (game.state) {
    case "timeout":
      return "Timeout";
    case "initial":
      return "Initial";
    case "finished":
      return "Finished";
  }
  let prefix = {
    noSetPlay: "",
    kickOff: "Kick-off",
    directFreeKick: "Direct Free Kick",
    indirectFreeKick: "Indirect Free Kick",
    penaltyKick: "Penalty Kick",
    throwIn: "Throw-in",
    goalKick: "Goal Kick",
    cornerKick: "Corner Kick",
  }[game.setPlay];
  let state = "";
  if (game.state === "ready") {
    state = " Ready";
  } else if (game.state === "set") {
    state = " Set";
  } else if (prefix === "") {
    state = "Playing";
  }
  return prefix + state;
};

const ClockPanel = ({ game, legalGameActions }) => {
  return (
    <div className="flex flex-col items-center h-(--clock-h)">
      <p className="h-6">{getPhaseDescription(game)}</p>
      <div className="relative">
        <p
          className={`tabular-nums text-clock font-medium ${
            game.primaryTimer.started
              ? game.primaryTimer.started.remaining[0] < 10
                ? "animate-flash-text"
                : ""
              : "invisible"
          }`}
        >
          {formatMMSS(game.primaryTimer)}
        </p>
        <div
          className={`absolute top-7 -right-8 ${
            game.phase === "penaltyShootout" ? "invisible" : ""
          }`}
        >
          <ActionButton
            action={{ type: "addAdditionalTime", args: null }}
            legal={legalGameActions[actions.ADD_ADDITIONAL_TIME]}
            label="+"
          />
        </div>
      </div>
      <p className="h-6">{getStateDescription(game)}</p>
      <p className={`tabular-nums text-heading ${game.secondaryTimer.started ? "" : "invisible"}`}>
        {formatMMSS(game.secondaryTimer)}
      </p>
    </div>
  );
};

export default ClockPanel;
