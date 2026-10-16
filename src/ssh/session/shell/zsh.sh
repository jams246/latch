cat > "$_latch_tmp/hooks" <<'LATCH_ZSH' || _latch_fallback
if [[ -z ${_latch_zsh_tmp-} ]]; then
    typeset -g _latch_zsh_tmp=$_LATCH_TMP _latch_zsh_restore=$_LATCH_RESTORE
    typeset -g _latch_zsh_zdot_set=$_LATCH_ZDOT_SET _latch_zsh_zdot=$_LATCH_ZDOT_VALUE
    unset _LATCH_TMP _LATCH_RESTORE _LATCH_ZDOT_SET _LATCH_ZDOT_VALUE
fi
typeset -g _latch_zsh_stage=$1
set --
if [[ -n $_latch_zsh_zdot_set ]]; then
    export ZDOTDIR=$_latch_zsh_zdot
else
    unset ZDOTDIR
fi
typeset -g _latch_zsh_file=${ZDOTDIR-$HOME}/$_latch_zsh_stage
if [[ -f $_latch_zsh_file && -r $_latch_zsh_file ]]; then
    builtin source -- "$_latch_zsh_file"
fi
_latch_zsh_zdot_set=${ZDOTDIR+x}
_latch_zsh_zdot=${ZDOTDIR-}
if [[ $_latch_zsh_stage == .zlogin || ! -o rcs ]]; then
    _latch_restore=$_latch_zsh_restore
@COMMON@
    _latch_restore_cwd
    typeset -ga precmd_functions chpwd_functions
    precmd_functions+=(_latch_report_cwd)
    chpwd_functions+=(_latch_report_cwd)
    _latch_report_cwd
    command rm -f -- "$_latch_zsh_tmp/hooks" "$_latch_zsh_tmp/.zshenv" "$_latch_zsh_tmp/.zprofile" "$_latch_zsh_tmp/.zshrc" "$_latch_zsh_tmp/.zlogin"
    command rmdir -- "$_latch_zsh_tmp" 2>/dev/null || :
    unset _latch_zsh_tmp _latch_zsh_restore _latch_zsh_zdot_set _latch_zsh_zdot _latch_zsh_stage _latch_zsh_file _latch_restore
    unset -f _latch_restore_cwd
else
    export ZDOTDIR=$_latch_zsh_tmp
fi
LATCH_ZSH
for _latch_stage in .zshenv .zprofile .zshrc .zlogin; do
    printf '%s\n' "builtin source -- \"\${_latch_zsh_tmp-\$_LATCH_TMP}/hooks\" $_latch_stage" > "$_latch_tmp/$_latch_stage" || _latch_fallback
done
umask "$_latch_umask"
_LATCH_TMP=$_latch_tmp _LATCH_RESTORE=$_latch_restore _LATCH_ZDOT_SET=${ZDOTDIR+x} _LATCH_ZDOT_VALUE=${ZDOTDIR-} ZDOTDIR=$_latch_tmp "$_latch_shell" -l -i
exit $?
