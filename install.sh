#!/usr/bin/env bash
# Recto installer: packages, the shell, the theme, and the session.
#
#   ./install.sh                  check packages, build, show what will change, then install
#   ./install.sh --dry-run        only check and show; change nothing
#   ./install.sh --yes            do not ask (for scripts)
#   ./install.sh --with-suggested also install the suggested packages (Arch)
#   ./install.sh --no-packages    skip the package step
#   ./install.sh --uninstall      remove Recto and put back the configs it replaced
#
# Run it as yourself, not as root; it asks sudo only to install packages.
# Existing configs are never overwritten: they are renamed *.recto-backup-<time>.

set -euo pipefail

cd "$(dirname "$0")"
REPO=$(pwd)
THEME="$REPO/tools/recto-theme"

# Arch package names (docs/THEMES.md lists what each is for).
REQUIRED=(niri foot swaybg swaylock swayidle librsvg python libpulse
          gtk4 gtk4-layer-shell rust
          adobe-source-serif-fonts inter-font ttf-ibm-plex
          mako fuzzel)
SUGGESTED=(adw-gtk-theme qt6ct polkit-gnome upower networkmanager fcitx5
           xdg-desktop-portal-gnome xdg-desktop-portal-gtk xwayland-satellite
           wireplumber brightnessctl)

YES=0 DRY=0 PACKAGES=1 SUGGEST=0 UNINSTALL=0
for arg in "$@"; do
    case "$arg" in
        -y|--yes) YES=1 ;;
        --dry-run) DRY=1 ;;
        --no-packages) PACKAGES=0 ;;
        --with-suggested) SUGGEST=1 ;;
        --uninstall) UNINSTALL=1 ;;
        -h|--help) sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "install.sh: unknown option '$arg' (see --help)" >&2; exit 2 ;;
    esac
done

say()  { printf '\n\033[1m%s\033[0m\n' "$*"; }
note() { printf '  %s\n' "$*"; }
stop() { printf '\ninstall.sh: %s\n' "$*" >&2; exit 1; }

# Ask a yes/no question; Enter means yes. --yes answers for you, --dry-run says no.
ask() {
    if (( DRY )); then return 1; fi
    if (( YES )); then return 0; fi
    local reply
    read -r -p "$1 [Y/n] " reply || return 1
    [[ -z "$reply" || "$reply" == [Yy]* ]]
}

(( EUID != 0 )) || stop "run this as your own user, not as root."

# ── Uninstall ──────────────────────────────────────────────────────────────
if (( UNINSTALL )); then
    say "Uninstall"
    python3 "$THEME" uninstall --dry-run | sed 's/^/  /'
    if [[ -n "${NIRI_SOCKET:-}" ]]; then
        note ""
        note "You are inside a niri session. It keeps running with its current"
        note "settings; log out afterwards (or uninstall from another session)."
    fi
    if ask "Remove Recto as listed above?"; then
        python3 "$THEME" uninstall | sed 's/^/  /'
        say "Done. Packages stay installed; remove them with pacman if you like."
    else
        note "Nothing was changed."
    fi
    exit 0
fi

# ── 1. Packages ────────────────────────────────────────────────────────────
say "1. Packages"
python3 -c 'import sys; sys.exit(sys.version_info < (3, 11))' 2>/dev/null \
    || stop "Python 3.11 or newer is needed (python3 not found or too old)."

if (( PACKAGES )) && command -v pacman >/dev/null; then
    wanted=("${REQUIRED[@]}")
    # rustup provides cargo too, and conflicts with the rust package.
    if command -v cargo >/dev/null; then
        kept=()
        for pkg in "${wanted[@]}"; do [[ "$pkg" == rust ]] || kept+=("$pkg"); done
        wanted=("${kept[@]}")
    fi
    (( SUGGEST )) && wanted+=("${SUGGESTED[@]}")
    mapfile -t missing < <(pacman -T "${wanted[@]}" 2>/dev/null | grep -v '^$' || true)
    if (( ${#missing[@]} == 0 )); then
        note "Everything needed is installed."
    else
        note "Missing: ${missing[*]}"
        if ask "Install them with: sudo pacman -S --needed ${missing[*]} ?"; then
            sudo pacman -S --needed "${missing[@]}"
        elif (( ! DRY )); then
            stop "these packages are needed; install them and run this again."
        fi
    fi
    if (( ! SUGGEST )); then
        mapfile -t extra < <(pacman -T "${SUGGESTED[@]}" 2>/dev/null | grep -v '^$' || true)
        if (( ${#extra[@]} )); then
            note "Suggested, not installed: ${extra[*]}"
            note "(./install.sh --with-suggested adds them: the GTK 3 and Qt themes,"
            note " the password dialog, battery and network status, input methods...)"
        fi
    fi
elif (( PACKAGES )); then
    # Not Arch: check for the programs and libraries instead of package names.
    absent=()
    for cmd in niri foot swaybg swaylock swayidle rsvg-convert pactl cargo pkg-config fc-list; do
        command -v "$cmd" >/dev/null || absent+=("$cmd")
    done
    if command -v pkg-config >/dev/null; then
        pkg-config --exists gtk4 || absent+=("gtk4 (development files)")
        pkg-config --exists gtk4-layer-shell-0 || absent+=("gtk4-layer-shell (development files)")
    fi
    if command -v fc-list >/dev/null; then
        for font in "Source Serif 4" "Inter" "IBM Plex Mono"; do
            fc-list : family | grep -qi "^$font" || absent+=("font: $font")
        done
    fi
    if (( ${#absent[@]} )); then
        note "Not found: ${absent[*]}"
        stop "install these with your distribution's package manager, then run this again."
    fi
    note "Everything needed was found."
else
    note "Skipped (--no-packages)."
fi

# ── 2. The shell ───────────────────────────────────────────────────────────
say "2. The shell (recto-shell)"
if (( DRY )); then
    note "Would build it with: cargo build --release --locked (in shell/)"
else
    (cd shell && cargo build --release --locked)
fi

# ── 3. The theme and configs ───────────────────────────────────────────────
say "3. Theme and configs"
python3 "$THEME" build | sed 's/^/  /'
note ""
note "This is what installing changes:"
python3 "$THEME" install --dry-run | sed 's/^/  /'
# Already in use (an update), or coming from another desktop (a first install)?
data="${XDG_DATA_HOME:-$HOME/.local/share}/recto"
niri_conf="${XDG_CONFIG_HOME:-$HOME/.config}/niri/config.kdl"
UPDATE=0
[[ -L "$niri_conf" && "$(readlink "$niri_conf")" == "$data/"* ]] && UPDATE=1
if ask "Install as listed above?"; then
    python3 "$THEME" install | sed 's/^/  /'
else
    note "Nothing was installed."
    exit 0
fi

# ── 4. Start ───────────────────────────────────────────────────────────────
say "4. Start"
if [[ -n "${NIRI_SOCKET:-}" ]] && (( ! UPDATE )); then
    # Switching from another desktop: its bar, wallpaper and launcher are still
    # running in this session, and niri has already picked up Recto's colors and
    # keys. Only a new login starts Recto cleanly (and stops the old ones).
    note "Recto is installed. niri has picked up its colors and keys already, but"
    note "the desktop you came from is still running in this session."
    note "Log out (Super+Shift+E) and log in again to start Recto."
elif [[ -n "${NIRI_SOCKET:-}" ]]; then
    if ask "You are in Recto already. Reload its config and restart the shell now?"; then
        niri msg action load-config-file || true
        "$data/bin/recto-shell" quit 2>/dev/null || true
        sleep 0.5
        setsid "$data/bin/recto-shell" >/dev/null 2>&1 < /dev/null &
        "$data/bin/recto-theme" session-start || true
        note "Reloaded. The idle lock and the password dialog start with the next login."
    fi
elif systemctl --user -q is-active niri.service 2>/dev/null; then
    # niri reloads its config file by itself, but runs spawn-at-startup only when
    # it starts: a session that was already running gets Recto's look, not its
    # bar, wallpaper or idle lock.
    note "A niri session is already running for this account. It has picked up"
    note "Recto's colors and keys, but the bar, wallpaper and idle lock start only"
    note "when niri starts: log out of it (Super+Shift+E) and log in again."
else
    note "Log out, then choose \"niri\" in your login screen,"
    note "or run niri-session from a text console."
fi
note ""
note "Keys: Super+Return terminal · Super+Space search · Super+A Today"
note "      Super+Shift+T Paper/Ink · Super+Alt+L lock · Super+Shift+/ all keys"
note "Your own niri settings (monitors, keyboard layout) go in ~/.config/niri/local.kdl."
note "To undo everything: ./install.sh --uninstall"
