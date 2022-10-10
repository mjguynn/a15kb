import QtQuick 2.15
import QtQuick.Layouts 1.2

import org.kde.plasma.components 3.0 as PlasmaComponents
import org.kde.plasma.core 2.1 as PlasmaCore
import org.kde.plasma.extras 2.0 as PlasmaExtras

import "logic.js" as Logic

RowLayout {
    id: root

    required property int temperature

    required property string deviceName

    spacing: PlasmaCore.Units.smallSpacing

    PlasmaCore.IconItem {
        source: Logic.iconForTemp(root.temperature)
        Layout.alignment: Qt.AlignTop
        Layout.preferredWidth: PlasmaCore.Units.iconSizes.medium
        Layout.preferredHeight: PlasmaCore.Units.iconSizes.medium
    }

    PlasmaComponents.Label {
        text: root.deviceName + ": "
        font.pixelSize: PlasmaCore.Units.iconSizes.small
    }
    PlasmaComponents.Label {
        text: Logic.stringForTemp(root.temperature)
        color: PlasmaCore.Theme.neutralTextColor
    }

}