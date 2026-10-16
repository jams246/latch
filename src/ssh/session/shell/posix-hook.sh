_latch_report_cwd() {
    local _latch_status=$?
    local LC_ALL=C _latch_text=$PWD _latch_char
    builtin printf '\033]7;file://'
    while [ -n "$_latch_text" ]; do
        _latch_char=${_latch_text%"${_latch_text#?}"}
        case "$_latch_char" in
            [a-zA-Z0-9/._~-]) builtin printf '%s' "$_latch_char" ;;
            *) builtin printf '%%%02X' "'$_latch_char" ;;
        esac
        _latch_text=${_latch_text#?}
    done
    builtin printf '\007'
    return @STATUS@
}
_latch_restore_cwd() {
    if [ -n "$_latch_restore" ] && ! builtin cd -- "$_latch_restore" 2>/dev/null; then
        builtin printf '%s\r\n' 'Latch: unable to restore working directory; using home.' >&2
        builtin cd -- "$HOME" 2>/dev/null || :
    fi
}
