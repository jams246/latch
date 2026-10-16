_latch_shell=$1
_latch_restore=$2
_latch_umask=$(umask)
umask 077
_latch_tmp=$(mktemp -d "${TMPDIR:-/tmp}/latch-shell.XXXXXXXX") || {
    umask "$_latch_umask"
    exec "$_latch_shell" -l -i
}
_latch_cleanup() {
    command rm -f -- "$_latch_tmp/startup" "$_latch_tmp/hooks" "$_latch_tmp/.zshenv" "$_latch_tmp/.zprofile" "$_latch_tmp/.zshrc" "$_latch_tmp/.zlogin"
    command rmdir -- "$_latch_tmp" 2>/dev/null || :
}
_latch_fallback() {
    _latch_cleanup
    umask "$_latch_umask"
    trap - 0 1 2 15
    exec "$_latch_shell" -l -i
}
trap '_latch_cleanup' 0
trap 'exit 129' 1
trap 'exit 130' 2
trap 'exit 143' 15
@STARTUP@
