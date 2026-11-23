# The log analysis tool.

stage_logs() {
  logs_dir="${distdir}/game_controller_logs-${version}-${label}"
  rm -rf "${logs_dir}"
  mkdir -p "${logs_dir}"
  cp "${basedir}/LICENSE" "${logs_dir}"
  write_third_party_licenses game_controller_logs "${logs_dir}/THIRD-PARTY-LICENSES"
  install_built game_controller_logs "${logs_dir}/game_controller_logs"
}

pack_logs() {
  make_tarball "${logs_dir}"
}
