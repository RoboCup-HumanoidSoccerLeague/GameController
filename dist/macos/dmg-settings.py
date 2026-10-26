# Settings of the disk image for dmgbuild (https://dmgbuild.readthedocs.io), with the defines app
# and background (see product-app.sh). The layout is the usual one: the app on the left,
# the link to /Applications on the right and an arrow in the background in between.

import os

app = defines["app"]

format = "UDZO"
files = [app]
symlinks = {"Applications": "/Applications"}
background = defines["background"]
icon_size = 128
text_size = 13
window_rect = ((200, 120), (540, 380))
default_view = "icon-view"
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
icon_locations = {
    os.path.basename(app): (140, 170),
    "Applications": (400, 170),
}
