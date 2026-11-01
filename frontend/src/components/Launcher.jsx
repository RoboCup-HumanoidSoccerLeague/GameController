import { useEffect, useState } from "react";
import CompetitionSettings from "./launcher/CompetitionSettings";
import GameSettings from "./launcher/GameSettings";
import { CastingSettings, InterfaceSettings } from "./launcher/NetworkSettings";
import TestSettings from "./launcher/TestSettings";
import WindowSettings from "./launcher/WindowSettings";
import { BUTTON_CLASSES, buttonStateClasses } from "./buttonClasses";
import { getLaunchData, launch } from "../api";

const Launcher = ({ setLaunched }) => {
  const [competitions, setCompetitions] = useState(null);
  const [launchSettings, setLaunchSettings] = useState(null);
  const [networkInterfaces, setNetworkInterfaces] = useState(null);
  const [teams, setTeams] = useState(null);

  const launchSettingsAreLegal =
    launchSettings != null &&
    launchSettings.game.teams.home.number != launchSettings.game.teams.away.number &&
    launchSettings.game.teams.home.fieldPlayerColor !=
      launchSettings.game.teams.away.fieldPlayerColor &&
    launchSettings.game.teams.home.fieldPlayerColor !=
      launchSettings.game.teams.away.goalkeeperColor &&
    launchSettings.game.teams.away.fieldPlayerColor !=
      launchSettings.game.teams.home.goalkeeperColor;

  useEffect(() => {
    getLaunchData().then((data) => {
      setCompetitions(data.competitions);
      setLaunchSettings(data.defaultSettings);
      setNetworkInterfaces(data.networkInterfaces);
      setTeams(data.teams);
    });
  }, []);

  if (
    competitions != null &&
    launchSettings != null &&
    networkInterfaces != null &&
    teams != null
  ) {
    const setCompetition = (competition) => {
      const INVISIBLES_NUMBER = 0;
      const defaultTeam = teams.find((team) => team.number === INVISIBLES_NUMBER); // Assuming that the Invisibles are part of every competition.
      setLaunchSettings({
        ...launchSettings,
        competition: competition,
        game: {
          ...launchSettings.game,
          teams: {
            home: {
              number: defaultTeam.number,
              fieldPlayerColor: defaultTeam.fieldPlayerColors[0],
              goalkeeperColor: defaultTeam.goalkeeperColors[0],
            },
            away: {
              number: defaultTeam.number,
              fieldPlayerColor: defaultTeam.fieldPlayerColors[1],
              goalkeeperColor: defaultTeam.goalkeeperColors[1],
            },
          },
        },
      });
    };
    const thisCompetition = competitions.find(
      (competition) => competition.id === launchSettings.competition.id,
    );
    const teamsInThisCompetition = teams.filter((team) =>
      thisCompetition.teams.includes(team.number),
    );
    const setNetwork = (network) => setLaunchSettings({ ...launchSettings, network: network });
    return (
      <div className="flex h-screen overflow-auto">
        <div className="m-auto flex h-full max-h-[35rem] min-h-max w-full max-w-[40rem] flex-col gap-3 p-4">
          <CompetitionSettings
            competitions={competitions}
            competition={launchSettings.competition}
            setCompetition={setCompetition}
          />
          <GameSettings
            teams={teamsInThisCompetition}
            game={launchSettings.game}
            setGame={(game) => setLaunchSettings({ ...launchSettings, game: game })}
          />
          <div className="grid grid-cols-[minmax(0,1fr)_5rem_minmax(0,1fr)] items-start gap-x-3 gap-y-2">
            <div className="col-start-1">
              <InterfaceSettings
                interfaces={networkInterfaces}
                network={launchSettings.network}
                setNetwork={setNetwork}
              />
            </div>
            <div className="col-start-3">
              <WindowSettings
                window={launchSettings.window}
                setWindow={(window) => setLaunchSettings({ ...launchSettings, window: window })}
              />
            </div>
            <div className="col-start-1 h-[5.5rem]">
              <CastingSettings network={launchSettings.network} setNetwork={setNetwork} />
            </div>
            <div className="col-start-3 h-[5.5rem]">
              <TestSettings
                test={launchSettings.game.test}
                setTest={(test) =>
                  setLaunchSettings({
                    ...launchSettings,
                    game: { ...launchSettings.game, test: test },
                  })
                }
              />
            </div>
          </div>
          <div className="flex flex-row justify-center">
            <button
              className={`px-8 py-2 text-xl font-semibold ${BUTTON_CLASSES} ${buttonStateClasses({ legal: launchSettingsAreLegal })}`}
              disabled={!launchSettingsAreLegal}
              onClick={() => launch(launchSettings).then(() => setLaunched(true))}
            >
              Start
            </button>
          </div>
        </div>
      </div>
    );
  } else {
    return <></>;
  }
};

export default Launcher;
