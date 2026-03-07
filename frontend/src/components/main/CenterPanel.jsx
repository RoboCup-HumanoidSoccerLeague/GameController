import ActionButton from "./ActionButton";
import ClockPanel from "./ClockPanel";
import PenaltyPanel from "./PenaltyPanel";
import StatePanel from "./StatePanel";
import * as actions from "../../actions.js";

const CenterPanel = ({
  game,
  legalGameActions,
  legalPenaltyActions,
  params,
  selectedPenaltyCall,
  setSelectedPenaltyCall,
}) => {
  return (
    <div className="grow flex flex-col gap-4">
      <ClockPanel game={game} legalGameActions={legalGameActions} />
      <div className="flex-1">
        <ActionButton
          action={{ type: "stopPlay", args: { resume: game.stopped } }}
          active={game.stopped}
          label={game.stopped ? "Resume Play" : "Stop Play"}
          legal={legalGameActions[game.stopped ? actions.RESUME_PLAY : actions.STOP_PLAY]}
        />
      </div>
      <StatePanel game={game} params={params} legalGameActions={legalGameActions} />
      <PenaltyPanel
        legalPenaltyActions={legalPenaltyActions}
        selectedPenaltyCall={selectedPenaltyCall}
        setSelectedPenaltyCall={setSelectedPenaltyCall}
      />
    </div>
  );
};

export default CenterPanel;
