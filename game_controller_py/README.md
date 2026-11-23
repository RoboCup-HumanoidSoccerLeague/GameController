# GameController for Python

This package exposes the GameController and its automatic referee to Python, e.g. for simulators. It is the Python counterpart of the C API in `game_controller_api`.

## Installation

Building requires a Rust toolchain and libclang. The package supports Python 3.9 or newer (a single `abi3` wheel per platform).

```bash
pip install ./game_controller_py                       # build and install into the current environment
maturin develop -m game_controller_py/Cargo.toml       # or: during development, in a virtual environment
maturin build -r -m game_controller_py/Cargo.toml      # or: build a wheel into game_controller_py/target/wheels
```

The crate is not a member of the root Cargo workspace, so that the rest of the GameController doesn't depend on PyO3.

## Usage

```python
from pathlib import Path
from game_controller import *

competition = CompetitionParams(Path("config/small_advanced/params.yaml").read_text())  # reusable
params = Params(
    competition,
    TeamParams(1, Color.BLUE, Color.YELLOW),
    TeamParams(2, Color.RED, Color.BLACK),
    Side.HOME,  # kick-off side
    SideMapping.HOME_DEFENDS_LEFT_GOAL,
)
gc = GameController(params)
autoreferee = Autoreferee(
    Features.ALL & ~Features.PLACE_FOR_PENALTY,
    FieldDimensions(
        length=9.0, width=6.0, goal_width=2.6, goal_height=1.2, goal_area_length=1.0,
        goal_area_width=4.0, penalty_area_length=2.0, penalty_area_width=5.0,
        penalty_mark_distance=1.5, center_circle_radius=0.75, border_strip_width=1.0,
        mark_radius=0.05, line_width=0.05, ball_radius=0.05,
    ),
    time_until_switch_to_set=2.0,
    time_until_switch_to_playing=1.0,
    time_until_resume=3.0,  # play stays stopped this long after a throw-in etc. was awarded
    time_until_ball_free=1.0,  # the set play ends this long after the ball clearly moved after the kick
    penalty_placement_distance=0.3,
)

gc.apply(StartSetPlay(SetPlay.KICK_OFF, side=Side.HOME))

# In each simulation step:
gc.seek(0.01)  # seconds
for command in autoreferee.update(gc, (0.0, 0.0, 0.05), {(Side.HOME, 1): PlayerPose(-1.0, 0.0, 0.0, 0.2)}):
    if isinstance(command, Command.PlaceBall):
        ...  # move the ball to (command.x, command.y, command.z)
    elif isinstance(command, Command.PlacePlayer):
        ...  # move player command.player of command.side to (command.x, command.y, command.theta)
    elif isinstance(command, Command.PlaceForWalkIn):
        ...  # put command.player of command.side to its walk-in pose
    elif isinstance(command, Command.WalkInComplete):
        ...  # all walk-in placements have been made, it is safe to switch to Ready etc.
    elif isinstance(command, Command.Whistle):
        ...
data = gc.control_message()  # the bytes of a RoboCupGameControlData struct, as sent to the players
```

- **Actions:** each action is a class, e.g. `Goal(side)`, `Penalize(side, player, call)` or `StopPlay(resume=False)`. All of them are subclasses of `Action`. `gc.apply(action, source=ActionSource.USER)` returns whether the action was legal (and thus applied), and `gc.is_legal(action)` only checks.
- **Ball contacts:** call `autoreferee.ball_contact(gc, side, player)` in every step in which a player touches the ball.
- **Coordinates and units:** see the crate `game_controller_autoreferee`. Lengths are in meters, angles in radians, times in seconds.
- **Errors:** invalid arguments (player numbers outside 1-20, unknown feature bits, competition parameters that can't be parsed, negative durations) raise `ValueError`.
