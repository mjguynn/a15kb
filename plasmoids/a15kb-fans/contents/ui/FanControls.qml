import QtQuick 2.15
import QtQuick.Layouts 1.2

import org.kde.plasma.components 3.0 as PlasmaComponents
import org.kde.plasma.core 2.1 as PlasmaCore
import org.kde.plasma.extras 2.0 as PlasmaExtras

import "logic.js" as Logic

ColumnLayout {
    anchors {
        left: parent.left
        right: parent.right
    }

	spacing: PlasmaCore.Units.smallSpacing

    PlasmaExtras.Heading {
        text: i18n("Fan Configuration")
    }

    Separator {}

    ColumnLayout {
    }
}