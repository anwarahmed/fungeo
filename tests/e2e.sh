#!/bin/sh
# End-to-end tests: the built program, run the way a user runs it.
#
#   tests/e2e.sh [path to the fungeo binary]     default: target/release/fungeo
#
#   1. the command line, and `fungeo check` on question files
#   2. install.sh and the self-updater, against releases made up here and served
#      from file:// (needs curl)
#   3. the game itself in detached tmux sessions (skipped when tmux is missing):
#      the intro, the passport, a whole round by keyboard, answers by mouse, a
#      player's own category, and the sounds (played by stand-ins that only note
#      what they were given)
#
# The questions, the rounds, the layout and the drawing are covered by `cargo test`;
# this is for what only shows when the real program meets a real terminal.
set -u

cd "$(dirname "$0")/.." || exit 1
ROOT=$PWD
BIN=${1:-target/release/fungeo}
case $BIN in /*) ;; *) BIN=$ROOT/$BIN ;; esac
[ -x "$BIN" ] || { echo "e2e.sh: $BIN is not built (cargo build --release)" >&2; exit 1; }

TMP=$(mktemp -d)
SOCK=fungeo-e2e-$$
trap 'tmux -L "$SOCK" kill-server 2>/dev/null; rm -rf "$TMP"' EXIT

fails=0
pass() { printf 'ok    %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1"; fails=$((fails + 1)); }
is() { # name expected actual
    if [ "$2" = "$3" ]; then pass "$1"; else fail "$1: expected '$2', got '$3'"; fi
}
has() { # name text-to-find text
    case $3 in *"$2"*) pass "$1" ;; *) fail "$1: no '$2' in '$3'" ;; esac
}

VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)

# ---------------------------------------------------------- command line ----

has "--version" "fungeo $VERSION (" "$("$BIN" --version)"
has "--help" "fungeo check" "$("$BIN" --help)"
has "an unknown option is refused" "unknown argument" "$("$BIN" --nonsense 2>&1)"
has "an unknown theme is refused" "--theme needs one of" "$("$BIN" --theme plaid 2>&1)"
has "needs a terminal" "needs a terminal" "$("$BIN" </dev/null 2>&1)"
has "a bad animations switch is refused" "--animations needs on or off" "$("$BIN" --animations sometimes 2>&1)"
XDG_STATE_HOME="$TMP/xdg-anim" "$BIN" --animations off </dev/null >/dev/null 2>&1
is "--animations off is remembered" "animations=0" "$(grep -x 'animations=0' "$TMP/xdg-anim/fungeo/settings" 2>/dev/null)"
has "a bad sound switch is refused" "--sound needs on or off" "$("$BIN" --sound loud 2>&1)"
XDG_STATE_HOME="$TMP/xdg-anim" "$BIN" --sound off </dev/null >/dev/null 2>&1
is "--sound off is remembered" "sound=0" "$(grep -x 'sound=0' "$TMP/xdg-anim/fungeo/settings" 2>/dev/null)"
has "a checkout never updates itself" "running from a source checkout" "$("$BIN" update 2>&1)"

# Question files of the player's own: a good one, and one with a mistake.
OWN=$TMP/config/fungeo/questions
mkdir -p "$OWN"
cat > "$OWN/town.txt" <<'QUESTIONS'
name: Our Town
order: 0

[easy]
Q: Which river runs through our town?
A: The Avon
X: The Nile | The Thames | The Amazon
F: It rises in the hills.

Q: What is our street called?
A: Mill Lane
X: High Street | Main Road | Park Avenue

Q: Which country do we live in?
A: England
X: France | Spain | Italy

Q: What is the nearest big city?
A: Bristol
X: Leeds | York | Dover
QUESTIONS
out=$(XDG_CONFIG_HOME="$TMP/config" "$BIN" check 2>&1)
has "check: lists the built-in categories" "Countries" "$out"
has "check: lists the player's own" "Our Town       4 easy, 0 medium, 0 hard" "$out"
is "check: a good file is fine" "0" "$(XDG_CONFIG_HOME="$TMP/config" "$BIN" check >/dev/null 2>&1; echo $?)"
has "check: a named file is checked" '"Our Town" is fine' "$("$BIN" check "$OWN/town.txt" 2>&1)"
printf '[easy]\nQ: broken?\nA: yes\n' > "$TMP/broken.txt"
out=$("$BIN" check "$TMP/broken.txt" 2>&1)
has "check: a mistake is named by line" "line 2: 'yes' needs three wrong answers" "$out"
is "check: a mistake fails the command" "1" "$("$BIN" check "$TMP/broken.txt" >/dev/null 2>&1; echo $?)"

# ------------------------------------------------- install and self-update ----

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

case "$(uname -s)-$(uname -m)" in
    Linux-x86_64 | Linux-amd64) ASSET=fungeo-x86_64-unknown-linux-musl ;;
    Linux-aarch64 | Linux-arm64) ASSET=fungeo-aarch64-unknown-linux-musl ;;
    Darwin-arm64) ASSET=fungeo-aarch64-apple-darwin ;;
    Darwin-x86_64) ASSET=fungeo-x86_64-apple-darwin ;;
    *) ASSET= ;;
esac

# make_release <dir> <version> <file to publish as this platform's binary>
make_release() {
    mkdir -p "$1"
    cp "$3" "$1/$ASSET"
    echo "$2" > "$1/VERSION"
    echo "$(sha256_of "$1/$ASSET")  $ASSET" > "$1/SHA256SUMS"
}

INST=$TMP/inst/bin/fungeo
installed() { XDG_STATE_HOME="$TMP/ustate" "$INST" "$@" 2>&1; }
# The copy a user would have: outside any checkout, in a directory they own.
fresh_copy() {
    rm -rf "$TMP/inst"
    mkdir -p "$TMP/inst/bin"
    cp "$BIN" "$INST"
}

if ! command -v curl >/dev/null 2>&1 || [ -z "$ASSET" ]; then
    echo "skip  install and self-update (no curl, or no release binary for this platform)"
else
    # A "newer release" whose binary is a script, so that it is plain which one runs.
    printf '#!/bin/sh\necho "fungeo 99.0.0 (fake)"\n' > "$TMP/fake"
    chmod 755 "$TMP/fake"
    make_release "$TMP/rel-now" "$VERSION" "$BIN"
    make_release "$TMP/rel-new" 99.0.0 "$TMP/fake"
    make_release "$TMP/rel-old" 0.0.1 "$TMP/fake"
    make_release "$TMP/rel-bad" 99.0.0 "$TMP/fake"
    echo "0000000000000000000000000000000000000000000000000000000000000000  $ASSET" > "$TMP/rel-bad/SHA256SUMS"

    out=$(FUNGEO_RELEASE_URL="file://$TMP/rel-now" FUNGEO_BIN_DIR="$TMP/inst/bin" sh install.sh 2>&1)
    has "install.sh: installs the release" "fungeo $VERSION (" "$(installed --version)"
    has "install.sh: says where" "Installed $INST" "$out"
    out=$(FUNGEO_RELEASE_URL="file://$TMP/rel-bad" FUNGEO_BIN_DIR="$TMP/inst2/bin" sh install.sh 2>&1)
    has "install.sh: refuses a bad checksum" "checksum mismatch" "$out"
    if [ -e "$TMP/inst2/bin/fungeo" ]; then fail "install.sh: installed despite a bad checksum"; else pass "install.sh: a refused download installs nothing"; fi
    FUNGEO_BIN_DIR="$TMP/inst/bin" sh install.sh --uninstall >/dev/null 2>&1
    if [ -e "$INST" ]; then fail "install.sh: --uninstall left the binary"; else pass "install.sh: --uninstall removes it"; fi

    fresh_copy
    has "update: nothing newer" "fungeo $VERSION is up to date (latest release is $VERSION)." "$(FUNGEO_RELEASE_URL="file://$TMP/rel-now" installed update)"
    has "update: never downgrades" "is up to date (latest release is 0.0.1)." "$(FUNGEO_RELEASE_URL="file://$TMP/rel-old" installed update)"
    has "update: refuses a bad checksum" "checksum mismatch" "$(FUNGEO_RELEASE_URL="file://$TMP/rel-bad" installed update)"
    has "update: a refused update changes nothing" "fungeo $VERSION (" "$(installed --version)"
    has "update: reports an unreachable server" "could not check for updates" "$(FUNGEO_RELEASE_URL="file://$TMP/nowhere" installed update)"

    # What a package does when it installs: a marker beside the binary's directory.
    mkdir -p "$TMP/inst/share/fungeo"
    echo "Homebrew; use brew upgrade fungeo" > "$TMP/inst/share/fungeo/managed-by"
    is "update: a package's copy refuses" "fungeo: this copy can't update itself: installed with Homebrew; use brew upgrade fungeo" "$(FUNGEO_RELEASE_URL="file://$TMP/rel-new" installed update)"
    has "update: a package's copy is untouched" "fungeo $VERSION (" "$(installed --version)"

    # Reached through a link, as Homebrew's bin directory does it: still refused.
    mkdir -p "$TMP/link"
    ln -s "$INST" "$TMP/link/fungeo"
    has "update: a package's copy refuses through a link too" "installed with Homebrew" "$(FUNGEO_RELEASE_URL="file://$TMP/rel-new" XDG_STATE_HOME="$TMP/ustate" "$TMP/link/fungeo" update 2>&1)"

    fresh_copy
    has "update: installs a newer release" "Updated to 99.0.0." "$(FUNGEO_RELEASE_URL="file://$TMP/rel-new" installed update)"
    is "update: the new version is what runs" "fungeo 99.0.0 (fake)" "$(installed --version)"

    # Through a link, the real file is replaced and the link is left alone.
    fresh_copy
    FUNGEO_RELEASE_URL="file://$TMP/rel-new" XDG_STATE_HOME="$TMP/ustate" "$TMP/link/fungeo" update >/dev/null 2>&1
    if [ -L "$TMP/link/fungeo" ] && [ "$("$INST" --version)" = "fungeo 99.0.0 (fake)" ]; then
        pass "update: through a link, the real file is replaced and the link survives"
    else
        fail "update: through a link, the link was replaced or the file was not"
    fi

    fresh_copy
    has "update off" "The update check at startup is off." "$(installed update off)"
    is "update off is remembered" "update=0" "$(grep -x 'update=0' "$TMP/ustate/fungeo/settings")"
    has "update on" "The update check at startup is on." "$(installed update on)"
fi

# -------------------------------------------------------------- the game ----

if ! command -v tmux >/dev/null 2>&1; then
    echo "skip  the game in tmux (tmux is not installed)"
else
    # Everything runs on a private tmux server, so no other session is ever touched.
    T() { tmux -L "$SOCK" "$@"; }
    screen() { T capture-pane -p -t main 2>/dev/null; }
    expect() { # name text -- waits up to 10 seconds for the text to be on screen
        i=0
        while [ "$i" -lt 100 ]; do
            if screen | grep -qF -- "$2"; then pass "$1"; return; fi
            sleep 0.1
            i=$((i + 1))
        done
        fail "$1: '$2' never appeared"
        screen | sed 's/^/        | /'
    }
    absent() { # name text
        if screen | grep -qF -- "$2"; then fail "$1: '$2' is on screen"; else pass "$1"; fi
    }
    keys() { T send-keys -t main "$@"; }
    # click <text>: a left click (SGR mouse press and release) on the first character
    # of the text, wherever it is on screen. Not every awk counts characters (the one
    # on macOS counts bytes, and a star is three), so the bytes that continue a
    # character are dropped first, from the screen and from the text alike, and
    # everything is then counted as bytes.
    click() {
        want=$(printf '%s' "$1" | LC_ALL=C tr -d '\200-\277')
        at=$(screen | LC_ALL=C tr -d '\200-\277' | LC_ALL=C awk -v t="$want" '{ i = index($0, t); if (i) { print i ";" NR; exit } }')
        [ -n "$at" ] || { fail "click: '$1' is not on screen"; return; }
        T send-keys -t main -l "$(printf '\033[<0;%sM\033[<0;%sm' "$at" "$at")"
    }
    # start <state dir> <command and arguments>: a session running the game, with the
    # player's own questions from above
    start() {
        state=$1
        shift
        T kill-session -t main 2>/dev/null
        T -f /dev/null new-session -d -s main -x 80 -y 24 \
            "env XDG_STATE_HOME='$state' XDG_CONFIG_HOME='$TMP/config' FUNGEO_NO_UPDATE=1 FUNGEO_NO_SOUND=1 $*; echo \"EXIT=\$?\"; sleep 20"
    }
    # Guesses A until the bridge is built. A script cannot know the right answer, and
    # a wrong one only sends the question round again, so this always gets there.
    build_the_bridge() {
        tries=0
        while [ "$tries" -lt 300 ]; do
            keys a
            i=0
            until screen | grep -qE 'Next|Cross!' || [ "$i" -ge 50 ]; do sleep 0.1; i=$((i + 1)); done
            if screen | grep -qF 'Cross!'; then return 0; fi
            keys Enter
            i=0
            until screen | grep -qF 'Click an answer' || [ "$i" -ge 50 ]; do sleep 0.1; i=$((i + 1)); done
            tries=$((tries + 1))
        done
        return 1
    }

    # The intro plays by itself and waits for nobody, but any key ends it.
    start "$TMP/xdg" "'$BIN'"
    expect "intro: it plays" "Press any key or click to start"
    keys x
    expect "passport: any key ends the intro" "0 of 48 stars"
    expect "passport: the player's own category is there" "Our Town"
    expect "passport: and the built-in ones" "Wonders"

    # The settings, by key and by mouse, and that they are remembered.
    keys t
    expect "passport: T changes the theme" "T ocean"
    is "passport: the theme is remembered" "theme=ocean" "$(grep -x 'theme=ocean' "$TMP/xdg/fungeo/settings" 2>/dev/null)"
    click "T ocean"
    expect "mouse: a click changes the theme" "T candy"
    keys m
    expect "passport: M switches the animations off" "M Motion off"
    is "passport: animations off is remembered" "animations=0" "$(grep -x 'animations=0' "$TMP/xdg/fungeo/settings" 2>/dev/null)"
    click "M Motion"
    sleep 0.3
    absent "mouse: a click switches them on again" "Motion off"
    keys '?'
    expect "help: opens" "Build a bridge across the river!"
    keys x
    sleep 0.3
    absent "help: any key closes it" "Build a bridge across the river!"
    keys Escape
    sleep 0.5
    absent "passport: Esc does not quit" "EXIT="

    # A round by keyboard: down a category, right a level, Enter.
    keys Down Down Right Enter
    expect "round: the arrows and Enter choose a category and level" "Flags · Medium · 0 of 8 planks"
    expect "round: a flag question asks whose it is" "Whose flag is this?"
    keys Escape
    expect "round: Esc goes back to the passport" "of 48 stars"
    # The same square with vim's keys: from Flags · Medium, up and down again, left and right again.
    keys k j h l Enter
    expect "round: H J K L move the marker as the arrows do" "Flags · Medium · 0 of 8 planks"
    keys Escape
    expect "round: Esc goes back to the passport again" "of 48 stars"
    sleep 0.3
    # The player's own category sorts first (order: 0), and has only an Easy level.
    keys Up Up Left Enter
    expect "own: the player's own category starts" "Our Town · Easy · 0 of 8 planks"
    if build_the_bridge; then pass "own: four questions go round until the bridge is built"; else fail "own: the bridge was never built"; fi
    keys Escape
    expect "own: back at the passport" "of 48 stars"
    sleep 0.3

    # A whole round of a built-in category, to the stars.
    keys Down Enter
    expect "round: starts" "Countries · Easy · 0 of 8 planks"
    expect "round: says what to do" "Every right answer adds a plank"
    if build_the_bridge; then pass "round: eight right answers build the bridge"; else fail "round: the bridge was never built"; fi
    expect "round: the planks are counted" "8 of 8 planks"
    keys Enter
    expect "round: the explorer crosses and the stars come" "Play again"
    expect "round: the way back is offered" "Passport"
    keys Enter
    expect "round: Enter goes back to the passport" "of 48 stars"
    absent "round: the stars are on the passport" "0 of 48 stars"

    # The same by mouse: a square, an answer, Next, Back.
    click "☆ ☆ ☆"
    expect "mouse: a clicked square starts its round" "0 of 8 planks"
    click " B  "
    expect "mouse: a clicked answer is taken" "Next"
    click "Next"
    expect "mouse: Next brings the next question" "Click an answer"
    keys 3
    expect "round: a number answers too" "Next"
    click "Esc Back"
    expect "mouse: Back goes to the passport" "of 48 stars"

    T resize-window -t main -x 40 -y 8 2>/dev/null
    expect "window: a tiny one says so" "Please make the window bigger"
    T resize-window -t main -x 200 -y 55 2>/dev/null
    expect "window: a big one draws the names in big letters" "█▀▀▀"
    keys Enter
    expect "window: a big one starts a round" "0 of 8 planks"
    keys q
    expect "round: Q goes back to the passport" "of 48 stars"
    keys q
    expect "passport: Q quits cleanly" "EXIT=0"

    # Every start is a fresh passport: the stars earned above are gone.
    start "$TMP/xdg" "'$BIN'" --no-intro
    expect "stars: a new start has none" "0 of 48 stars"
    keys Enter
    expect "round: starts again" "0 of 8 planks"
    keys C-c
    expect "round: Ctrl-C quits at once" "EXIT=0"

    # Sound: the system's player is asked to play a file for the intro, for a new
    # round and for an answer. Stand-ins for every player it might look for note what
    # they were given.
    mkdir -p "$TMP/players"
    for player in pw-play paplay aplay afplay; do
        printf '#!/bin/sh\nfor a in "$@"; do echo "$a"; done >> "%s"\n' "$TMP/played" > "$TMP/players/$player"
        chmod 755 "$TMP/players/$player"
    done
    start "$TMP/xdg5" env -u FUNGEO_NO_SOUND "PATH='$TMP/players:$PATH'" "'$BIN'"
    expect "sound: the intro plays" "Press any key or click to start"
    keys x
    expect "sound: the passport comes" "0 of 48 stars"
    keys Down Enter
    expect "sound: a round starts" "0 of 8 planks"
    keys a
    expect "sound: an answer is taken" "Next"
    sleep 0.6
    played=$(cat "$TMP/played" 2>/dev/null)
    has "sound: the intro is heard" "$TMP/xdg5/fungeo/sounds/intro.wav" "$played"
    has "sound: a new round is heard" "$TMP/xdg5/fungeo/sounds/start.wav" "$played"
    case $played in
        *sounds/right.wav* | *sounds/wrong.wav*) pass "sound: an answer is heard" ;;
        *) fail "sound: no right.wav or wrong.wav in '$played'" ;;
    esac
    is "sound: what is played is a WAV file" "RIFF" "$(head -c 4 "$TMP/xdg5/fungeo/sounds/intro.wav" 2>/dev/null)"
    keys s
    sleep 0.5
    is "sound: S switches it off, and that is remembered" "sound=0" "$(grep -x 'sound=0' "$TMP/xdg5/fungeo/settings" 2>/dev/null)"
    rm -f "$TMP/played"
    keys Enter
    expect "sound: the next question comes with sound off" "Click an answer"
    keys a
    expect "sound: and is answered" "Next"
    sleep 0.6
    if [ -e "$TMP/played" ]; then fail "sound: something was played after S"; else pass "sound: switched off, nothing is played"; fi
    keys s
    sleep 0.6
    has "sound: switched on, it is heard at once" "click.wav" "$(cat "$TMP/played" 2>/dev/null)"
    keys C-c
    expect "sound: quits cleanly" "EXIT=0"

    # Starting the installed copy when a newer release exists: it updates and restarts
    # as the new version. Switched off, it starts the game without updating.
    if command -v curl >/dev/null 2>&1 && [ -n "$ASSET" ]; then
        STAMP=$TMP/ustate/fungeo/last-update-check
        launch() { # <release directory>
            T kill-session -t main 2>/dev/null
            T -f /dev/null new-session -d -s main -x 80 -y 24 \
                "env XDG_STATE_HOME='$TMP/ustate' XDG_CONFIG_HOME='$TMP/none' FUNGEO_NO_SOUND=1 FUNGEO_RELEASE_URL='file://$1' '$INST' --no-intro; echo \"EXIT=\$?\"; sleep 20"
        }
        fresh_copy
        rm -f "$STAMP"
        installed update off >/dev/null
        launch "$TMP/rel-new"
        expect "update: switched off, the game starts" "0 of 45 stars"
        keys q
        expect "update: switched off, it quits cleanly" "EXIT=0"
        has "update: switched off, nothing is updated" "fungeo $VERSION (" "$(installed --version)"
        if [ -e "$STAMP" ]; then fail "update: switched off, yet a check was noted"; else pass "update: switched off, nothing is checked"; fi
        installed update on >/dev/null

        # At most one check a day: a start that finds nothing newer notes the time, and
        # the next start does not look again, even with a newer release on offer.
        launch "$TMP/rel-now"
        expect "once a day: the game starts after a check that finds nothing" "0 of 45 stars"
        keys q
        expect "once a day: it quits cleanly" "EXIT=0"
        if [ -s "$STAMP" ]; then pass "once a day: the check is noted"; else fail "once a day: no $STAMP"; fi
        launch "$TMP/rel-new"
        expect "once a day: the next start goes straight to the game" "0 of 45 stars"
        keys q
        expect "once a day: and quits cleanly" "EXIT=0"
        has "once a day: no second check, so no update" "fungeo $VERSION (" "$(installed --version)"
        has "once a day: asking explicitly still checks" "Updating fungeo $VERSION -> 99.0.0" "$(FUNGEO_RELEASE_URL="file://$TMP/rel-bad" installed update)"

        # A day later (the noted time is old), a start checks again and updates.
        fresh_copy
        echo 1000 > "$STAMP"
        launch "$TMP/rel-new"
        expect "update: a day later, starting the game updates it and runs the new version" "fungeo 99.0.0 (fake)"
    fi
    T kill-server 2>/dev/null
fi

echo
if [ "$fails" -gt 0 ]; then
    echo "$fails failed"
    exit 1
fi
echo "all passed"
