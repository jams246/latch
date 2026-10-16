cat > "$_latch_tmp/startup" <<'LATCH_BASH' || _latch_fallback
_latch_tmp=$_LATCH_TMP
_latch_restore=$_LATCH_RESTORE
command rm -f -- "$_latch_tmp/startup"
command rmdir -- "$_latch_tmp" 2>/dev/null || :
unset _latch_tmp
if [ -n "$_LATCH_ENV_SET" ]; then
    export ENV=$_LATCH_ENV_VALUE
else
    unset ENV
fi
if [ -n "$_LATCH_HISTFILE_DEFAULT" ]; then
    export -n HISTFILE
fi
unset _LATCH_TMP _LATCH_RESTORE _LATCH_ENV_SET _LATCH_ENV_VALUE _LATCH_HISTFILE_DEFAULT
builtin set +o posix
builtin shopt -u inherit_errexit 2>/dev/null || :
if [ -f /etc/profile ] && [ -r /etc/profile ]; then
    builtin source /etc/profile
fi
for _latch_profile in "$HOME/.bash_profile" "$HOME/.bash_login" "$HOME/.profile"; do
    if [ -f "$_latch_profile" ] && [ -r "$_latch_profile" ]; then
        builtin source "$_latch_profile"
        break
    fi
done
@COMMON@
_latch_restore_cwd
case $(builtin declare -p PROMPT_COMMAND 2>/dev/null) in
    'declare -a'*) PROMPT_COMMAND+=('_latch_report_cwd') ;;
    *) PROMPT_COMMAND="${PROMPT_COMMAND-}"$'\n''_latch_report_cwd' ;;
esac
unset _latch_restore _latch_profile
unset -f _latch_restore_cwd
LATCH_BASH
if [ "${HISTFILE+x}" ]; then
    _latch_history=$HISTFILE
    _latch_default_history=
else
    _latch_history=$HOME/.bash_history
    _latch_default_history=1
fi
umask "$_latch_umask"
_LATCH_TMP=$_latch_tmp _LATCH_RESTORE=$_latch_restore _LATCH_ENV_SET=${ENV+x} _LATCH_ENV_VALUE=${ENV-} _LATCH_HISTFILE_DEFAULT=$_latch_default_history HISTFILE=$_latch_history ENV=$_latch_tmp/startup "$_latch_shell" --posix --login -i
exit $?
