import QtQuick 2.15
import QtQuick.Layouts 1.0

import org.kde.plasma.components 3.0 as PlasmaComponents
import org.kde.plasma.plasmoid 2.0

import com.offbyond.a15kb 1.0 as A15KB

import "logic.js" as Logic

Item {
    Plasmoid.icon: {
        let maxTemp = A15KB.Controller.cpuTemp;
        let gpuTemp = A15KB.Controller.gpuTemp;
        if (gpuTemp != 0) {
            maxTemp = Math.max(avgTemp + gpuTemp);
        }
        return Logic.iconForTemp(avgTemp);
    }
    Plasmoid.toolTipMainText: {
        return "CPU: " + Logic.stringForTemp(A15KB.Controller.cpuTemp);
    }
    Plasmoid.toolTipSubText: {
        return "GPU: " + Logic.stringForTemp(A15KB.Controller.gpuTemp);
    }

    PlasmaComponents.ScrollView {
        id: root

        focus: true
        anchors.fill: parent

        PlasmaComponents.ScrollBar.horizontal.policy: PlasmaComponents.ScrollBar.AlwaysOff

        contentItem: ListView {
            id: list
            keyNavigationEnabled: true

            leftMargin: PlasmaCore.Units.smallSpacing * 4
            rightMargin: PlasmaCore.Units.smallSpacing * 2
            topMargin: PlasmaCore.Units.smallSpacing * 2
            bottomMargin: PlasmaCore.Units.smallSpacing * 2
            spacing: PlasmaCore.Units.smallSpacing

            header: TemperatureContainer {}
        }
    }
}