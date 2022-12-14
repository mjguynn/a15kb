import QtQuick 2.15
import QtQuick.Layouts 1.2

import org.kde.plasma.components 3.0 as PlasmaComponents
import org.kde.plasma.core 2.1 as PlasmaCore
import org.kde.plasma.extras 2.0 as PlasmaExtras

import com.offbyond.a15kb 1.0 as A15KB

import "logic.js" as Logic

ColumnLayout {
    property alias title: heading.text
    default property alias data: content.data
    Layout.alignment: Qt.AlignTop
    spacing: PlasmaCore.Units.smallSpacing

    PlasmaExtras.Heading {
        id: heading
    }

    // Dividing line
    Rectangle {
        Layout.alignment: Qt.AlignLeft
        Layout.fillWidth: true
        Layout.preferredHeight: 1
        color: PlasmaCore.Theme.disabledTextColor
    }

    ColumnLayout {
        id: content
        spacing: PlasmaCore.Units.smallSpacing
        Layout.alignment: Qt.AlignLeft
        Layout.leftMargin: PlasmaCore.Units.smallSpacing * 4
    }
}