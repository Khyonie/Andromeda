alias edit="micro"

set -g theme_display_hostname yes
set -g theme_display_user yes
if status is-interactive
    # Commands to run in interactive sessions can go here
end

function __greeting_stars --argument-names length
    # Spaces are deliberately much more common.
    set -l glyphs \
        ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' ' \
        '·' '·' \
        '⋆' \
        '✧' \
        '✦'

    for i in (seq $length)
        printf '%s' $glyphs[(random 1 (count $glyphs))]
    end
end

function fish_greeting
    # Rose Pine
    set -l rose   ebbcba
    set -l pine   31748f
    set -l foam   9ccfd8
    set -l iris   c4a7e7
    set -l gold   f6c177
    set -l text   e0def4
    set -l subtle 908caa
    set -l muted  6e6a86
    set -l overlay 26233a

    set -l width 50

    set -l user $USER
    set -l host $hostname
    set -l linestart "▕"
    set -l kernel (uname -r)
	set -l distro (grep -oP '^PRETTY_NAME="\K[^"]+' /etc/os-release)
    set -l datetime (date "+%A, %B %d · %I:%M %p")

    echo

    # Pure star row
    set_color $iris
    echo "  "(__greeting_stars $width)

    # User / hostname
    set_color $gold
    printf '  %s' (__greeting_stars 5)
    set_color $rose
    printf ' %s ' "$linestart"

    set_color $foam
    printf '%s ' "$user"

    set_color $muted
    printf '@ '

    set_color $foam
    printf '%s' "$host"

    set -l used (math 5 + (string length "$user @ $host"))
    set_color $iris
    printf '%s\n' (__greeting_stars (math $width - $used))

    # OS / kernel
    set_color $iris
    printf '  %s' (__greeting_stars 5)
    set_color $rose
    printf ' %s ' "$linestart"

    set_color $text
    printf '%s' "$distro"

    set_color $muted
    printf ' · '

    set_color $pine
    printf '%s' "$kernel"

    set -l used (math 5 + (string length "Arch Linux · $kernel"))
    set_color $gold
    printf '%s\n' (__greeting_stars (math $width - $used))

    # Date / time
    set_color $gold
    printf '  %s' (__greeting_stars 5)
    set_color $rose
    printf ' %s ' "$linestart"

    set_color --italics $subtle
    printf '%s' "$datetime"

    set -l used (math 5 + (string length "$datetime"))
    set_color $iris
    printf '%s\n' (__greeting_stars (math $width - $used))

    # Pure star row
    set_color $gold
    echo "  "(__greeting_stars $width)

    # Bottom border
    set_color $iris
    echo ""(string repeat -n $width '─')

    set_color normal
end
