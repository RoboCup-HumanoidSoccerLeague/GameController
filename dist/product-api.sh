# The C API of the game controller core.

stage_api() {
  api_dir="${distdir}/game_controller_api-${version}-${label}"
  local library
  case "${target}" in
    *-apple-darwin) library=libgame_controller_api.dylib ;;
    *) library=libgame_controller_api.so ;;
  esac
  rm -rf "${api_dir}"
  mkdir -p "${api_dir}/include" "${api_dir}/lib" "${api_dir}/config"
  cp "${basedir}/LICENSE" "${api_dir}"
  cp "${basedir}/game_controller_api/headers/GameController.h" "${api_dir}/include"
  cp "${basedir}/game_controller_msgs/headers/RoboCupGameControlData.h" "${api_dir}/include"
  install_built "${library}" "${api_dir}/lib/${library}"
  local dir
  for dir in "${basedir}"/config/*; do
    [ -f "${dir}/params.yaml" ] || continue
    cp "${dir}/params.yaml" "${api_dir}/config/$(basename "${dir}").yaml"
  done
}

pack_api() {
  make_tarball "${api_dir}"
}
