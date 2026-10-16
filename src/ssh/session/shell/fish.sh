cat > "$_latch_tmp/startup" <<'LATCH_FISH' || _latch_fallback
function _latch_report_cwd --on-event fish_prompt --on-variable PWD
    set -l _latch_status $status
    set -l _latch_path (string escape --style=url -- "$PWD")
    printf '\033]7;file://%s\007' "$_latch_path"
    return $_latch_status
end
set -l _latch_tmp $_LATCH_TMP
set -l _latch_restore $_LATCH_RESTORE
set -e _LATCH_TMP
set -e _LATCH_RESTORE
if test -n "$_latch_restore"; and not builtin cd -- "$_latch_restore" 2>/dev/null
    printf '%s\r\n' 'Latch: unable to restore working directory; using home.' >&2
    builtin cd -- "$HOME" 2>/dev/null
end
command rm -f -- "$_latch_tmp/startup"
command rmdir -- "$_latch_tmp" 2>/dev/null
_latch_report_cwd
LATCH_FISH
umask "$_latch_umask"
_LATCH_TMP=$_latch_tmp _LATCH_RESTORE=$_latch_restore "$_latch_shell" -l -i -C 'source "$_LATCH_TMP/startup"'
exit $?
