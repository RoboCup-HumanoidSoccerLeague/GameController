# The GameController application: a prefix tree with install scripts on Linux, an app bundle in a
# disk image on macOS. (Windows is handled by mkdist.ps1.)

app_id=org.robocup.hsl.game-controller

stage_app() {
  case "${target}" in
    *-apple-darwin) stage_app_macos ;;
    *) stage_app_linux ;;
  esac
}

pack_app() {
  case "${target}" in
    *-apple-darwin) pack_app_macos ;;
    *) make_tarball "${app_dir}" ;;
  esac
}

# The archive has the layout of an installation prefix, so that it also runs in place, and the
# scripts install and uninstall copy the tree to a prefix and remove it again.
stage_app_linux() {
  app_dir="${distdir}/GameController-${version}-${target}"
  rm -rf "${app_dir}"
  mkdir -p "${app_dir}/bin" \
    "${app_dir}/share/applications" \
    "${app_dir}/share/icons/hicolor/512x512/apps" \
    "${app_dir}/share/doc/GameController" \
    "${app_dir}/share/GameController"
  install_built game_controller_app "${app_dir}/bin/GameController"
  cp -r "${basedir}/config" "${app_dir}/share/GameController"
  cp "${basedir}/LICENSE" "${basedir}/README.md" "${app_dir}/share/doc/GameController"
  cp "${basedir}/game_controller_app/icons/icon.png" \
    "${app_dir}/share/icons/hicolor/512x512/apps/${app_id}.png"
  # The Exec line is filled in by the install script. The app sets its window class / app ID to
  # GameController, which lets desktops associate it with this entry.
  cat >"${app_dir}/share/applications/${app_id}.desktop" <<END
[Desktop Entry]
Type=Application
Name=GameController
Comment=The GameController for the RoboCup Humanoid Soccer League
Exec=@EXEC@
Icon=${app_id}
Terminal=false
Categories=Utility;
StartupWMClass=GameController
END
  cp "${distdir}/linux/install" "${distdir}/linux/uninstall" "${app_dir}"
  chmod a+x "${app_dir}/install" "${app_dir}/uninstall"
}

stage_app_macos() {
  if ${universal}; then
    app_template="${distdir}/GameController-${version}"
    local archpref="
	<key>LSArchitecturePriority</key>
	<array>
		<string>arm64</string>
		<string>x86_64</string>
	</array>"
  else
    app_template="${distdir}/GameController-${version}-${target}"
    local archpref=
  fi
  app_dmg="${app_template}.dmg"
  local appdir="${app_template}/GameController.app"
  rm -rf "${app_template}"
  rm -f "${app_dmg}"

  # Everything the app needs is in the bundle, so that it can be copied around as a whole.
  mkdir -p "${appdir}/Contents/MacOS" "${appdir}/Contents/Resources"
  install_built game_controller_app "${appdir}/Contents/MacOS/GameController"
  cp -r "${basedir}/config" "${basedir}/LICENSE" "${basedir}/README.md" "${appdir}/Contents/Resources"
  cp "${distdir}/macos/allow-firewall" "${appdir}/Contents/Resources"
  cp "${basedir}/game_controller_app/icons/icon.icns" "${appdir}/Contents/Resources/GameController.icns"

  # The bundle versions must be numeric (X.Y.Z), which a release candidate is not.
  local short_version="${version%%-*}"
  local copyright
  copyright="$(sed -n 's/^\(Copyright.*\)$/\1/p' "${basedir}/LICENSE" | head -n 1)"
  cat >"${appdir}/Contents/Info.plist" <<END
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleExecutable</key>
	<string>GameController</string>
	<key>CFBundleIconFile</key>
	<string>GameController.icns</string>
	<key>CFBundleIdentifier</key>
	<string>${app_id}</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>GameController</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>${short_version}</string>
	<key>CFBundleSignature</key>
	<string>RGC3</string>
	<key>CFBundleSupportedPlatforms</key>
	<array>
		<string>MacOSX</string>
	</array>
	<key>CFBundleVersion</key>
	<string>${short_version}</string>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.utilities</string>
	<key>LSMinimumSystemVersion</key>
	<string>15.0</string>${archpref}
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSHumanReadableCopyright</key>
	<string>${copyright}</string>
	<key>NSLocalNetworkUsageDescription</key>
	<string>The GameController uses the local network to communicate with players and monitoring applications.</string>
	<key>NSPrincipalClass</key>
	<string>NSApplication</string>
</dict>
</plist>
END
  echo -n APPLRGC3 >"${appdir}/Contents/PkgInfo"

  # Without a Developer ID, the best that can be done is a self-signed certificate (see
  # macos/create-signing-certificate) that is used for all releases: the firewall and the local
  # network permission are tied to the identity (bundle identifier and certificate), so they stay
  # granted when the app is updated. CODESIGN_IDENTITY is the name or hash of the certificate and
  # CODESIGN_KEYCHAIN the keychain it is in. Without an identity, the app is signed ad hoc, which
  # gives a new identity with every build.
  local sign=(--force --deep --sign "${CODESIGN_IDENTITY:--}")
  if [ -n "${CODESIGN_IDENTITY}" ]; then
    sign+=(--timestamp=none)
    [ -z "${CODESIGN_KEYCHAIN}" ] || sign+=(--keychain "${CODESIGN_KEYCHAIN}")
  else
    >&2 echo "warning: CODESIGN_IDENTITY is not set, signing ad hoc"
  fi
  # The linker has already signed the binary; removing that signature avoids a warning.
  codesign --remove-signature "${appdir}/Contents/MacOS/GameController"
  codesign "${sign[@]}" "${appdir}"
  codesign --verify --deep --strict "${appdir}"

}

# The disk image is laid out (background with an arrow, icon positions) by dmgbuild, which writes
# the Finder settings directly instead of scripting a Finder window. It is installed on demand
# into a virtual environment.
pack_app_macos() {
  local venv="${distdir}/.venv-dmgbuild"
  if [ ! -x "${venv}/bin/dmgbuild" ]; then
    python3 -m venv "${venv}"
    "${venv}/bin/pip" install --quiet --disable-pip-version-check dmgbuild==1.6.5
  fi
  local background="${app_template}.background.tiff"
  tiffutil -cathidpicheck "${distdir}/macos/dmg-background.png" \
    "${distdir}/macos/dmg-background@2x.png" -out "${background}"
  # dmgbuild still uses the deprecated hdiutil verbs, which only produces warnings.
  "${venv}/bin/dmgbuild" -s "${distdir}/macos/dmg-settings.py" \
    -D "app=${app_template}/GameController.app" \
    -D "background=${background}" \
    "GameController ${version}" "${app_dmg}" 2> >(grep -v "^hdiutil: WARNING" >&2)
  rm -f "${background}"
}
