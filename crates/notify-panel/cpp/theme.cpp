#include "notify-panel/cpp/theme.h"

// uQ's shared KDE colour-scheme follower (uqkdetheme.*, vendored unchanged from uQ's
// native/uq-theme, checked by scripts/check-uq-native.sh).
#include "uqkdetheme.h"

#include <QString>

void notifi_follow_uq_theme()
{
    uqtheme::followKdeTheme(QStringLiteral("notify-panel"));
}
