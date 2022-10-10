import QtQuick 2.1

import org.kde.plasma.core 2.0 as PlasmaCore

Rectangle {
    anchors {
        left: parent.anchors.left
        topMargin: PlasmaCore.Units.smallSpacing
    }
    width: parent.width
    height: 1
    color: PlasmaCore.Theme.disabledTextColor
}